import Cocoa
import WebKit

final class NotchController: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    private let root: URL
    private let token = UUID().uuidString
    private let port: String
    private var gateway: Process?
    private var window: NSWindow!
    private var webView: WKWebView!

    init(root: URL, port: String) {
        self.root = root
        self.port = port
        super.init()
    }

    func start() {
        startGateway()

        let configuration = WKWebViewConfiguration()
        let bridge = "window.secondEgoWindow={setExpanded:function(expanded){window.webkit.messageHandlers.secondEgoWindow.postMessage(!!expanded);return Promise.resolve();}};"
        configuration.userContentController.addUserScript(
            WKUserScript(source: bridge, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        )
        configuration.userContentController.add(self, name: "secondEgoWindow")

        webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = self
        webView.autoresizingMask = [.width, .height]
        webView.setValue(false, forKey: "drawsBackground")

        window = NSWindow(
            contentRect: bounds(expanded: false),
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.isOpaque = true
        window.backgroundColor = NSColor(calibratedWhite: 0.035, alpha: 1.0)
        window.hasShadow = false
        window.level = .statusBar
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        window.isMovable = false
        window.hidesOnDeactivate = false
        webView.frame = NSRect(origin: .zero, size: bounds(expanded: false).size)
        window.contentView = webView
        NSApp.activate(ignoringOtherApps: true)
        NSRunningApplication.current.activate(options: [.activateAllWindows, .activateIgnoringOtherApps])
        window.makeKeyAndOrderFront(nil)
        window.orderFrontRegardless()

        let index = root.appendingPathComponent("apps/desktop/dist/index.html")
        var components = URLComponents(url: index, resolvingAgainstBaseURL: false)!
        components.queryItems = [
            URLQueryItem(name: "notch", value: "1"),
            URLQueryItem(name: "token", value: token),
            URLQueryItem(name: "gateway", value: "http://127.0.0.1:\(port)"),
        ]
        webView.loadFileURL(components.url!, allowingReadAccessTo: root.appendingPathComponent("apps/desktop/dist"))
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        NSLog("SecondEgo UI loaded")
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        NSLog("SecondEgo UI failed to load: %@", error.localizedDescription)
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        NSLog("SecondEgo UI failed before load: %@", error.localizedDescription)
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.name == "secondEgoWindow", let expanded = message.body as? Bool else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.window.setFrame(self.bounds(expanded: expanded), display: true, animate: true)
            NSApp.activate(ignoringOtherApps: true)
            NSRunningApplication.current.activate(options: [.activateAllWindows, .activateIgnoringOtherApps])
            self.window.orderFrontRegardless()
        }
    }

    func stop() {
        gateway?.terminate()
    }

    private func bounds(expanded: Bool) -> NSRect {
        let screen = NSScreen.main ?? NSScreen.screens[0]
        let screenFrame = screen.frame
        let width: CGFloat = expanded ? 920 : 236
        let height: CGFloat = expanded ? 820 : 38
        return NSRect(
            x: screenFrame.midX - width / 2,
            y: screenFrame.maxY - height,
            width: width,
            height: height
        )
    }

    private func startGateway() {
        let executable = root.appendingPathComponent("engine-rs/target/debug/secondego-gateway")
        guard FileManager.default.isExecutableFile(atPath: executable.path) else {
            NSLog("SecondEgo gateway not found at %@; run make desktop-electron again.", executable.path)
            return
        }
        let process = Process()
        process.executableURL = executable
        process.currentDirectoryURL = root
        var environment = ProcessInfo.processInfo.environment
        environment["SECONDEGO_UI_TOKEN"] = token
        environment["SECONDEGO_GATEWAY_PORT"] = port
        process.environment = environment
        process.standardOutput = FileHandle.standardOutput
        process.standardError = FileHandle.standardError
        do {
            try process.run()
            gateway = process
        } catch {
            NSLog("Could not start SecondEgo gateway: %@", error.localizedDescription)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var controller: NotchController!

    func applicationDidFinishLaunching(_ notification: Notification) {
        // A regular policy is required for reliable activation when launched
        // from a terminal on macOS. The window itself remains borderless and
        // floating at the notch; this is more important than hiding the Dock
        // icon during development.
        NSApp.setActivationPolicy(.regular)
        let rootPath = ProcessInfo.processInfo.environment["SECONDEGO_ROOT"] ?? findProjectRoot()
        let port = ProcessInfo.processInfo.environment["SECONDEGO_GATEWAY_PORT"] ?? "8787"
        controller = NotchController(root: URL(fileURLWithPath: rootPath, isDirectory: true), port: port)
        controller.start()
    }

    private func findProjectRoot() -> String {
        var candidate = Bundle.main.executableURL?.deletingLastPathComponent()
        while let directory = candidate {
            let gateway = directory.appendingPathComponent("engine-rs/target/debug/secondego-gateway")
            let index = directory.appendingPathComponent("apps/desktop/dist/index.html")
            if FileManager.default.isExecutableFile(atPath: gateway.path), FileManager.default.fileExists(atPath: index.path) {
                return directory.path
            }
            candidate = directory.deletingLastPathComponent()
            if candidate == directory { break }
        }
        return FileManager.default.currentDirectoryPath
    }

    func applicationWillTerminate(_ notification: Notification) {
        controller?.stop()
    }
}

let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
application.run()
