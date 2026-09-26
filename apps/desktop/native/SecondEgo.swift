import Cocoa
import Foundation
import WebKit

final class NotchWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

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
        let bridge = "window.secondEgoWindow={setExpanded:function(expanded){window.webkit.messageHandlers.secondEgoWindow.postMessage(!!expanded);return Promise.resolve();}};window.addEventListener('error',function(event){window.webkit.messageHandlers.secondEgoConsole.postMessage(String(event.message||'WebKit JavaScript error'));});window.addEventListener('unhandledrejection',function(event){window.webkit.messageHandlers.secondEgoConsole.postMessage(String(event.reason||'Unhandled promise rejection'));});"
        configuration.userContentController.addUserScript(
            WKUserScript(source: bridge, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        )
        configuration.userContentController.add(self, name: "secondEgoWindow")
        configuration.userContentController.add(self, name: "secondEgoConsole")

        webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = self
        webView.autoresizingMask = [.width, .height]
        webView.setValue(false, forKey: "drawsBackground")

        window = NotchWindow(
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

        var components = URLComponents(string: "http://127.0.0.1:\(port)/")!
        components.queryItems = [
            URLQueryItem(name: "notch", value: "1"),
            URLQueryItem(name: "token", value: token),
            URLQueryItem(name: "gateway", value: "http://127.0.0.1:\(port)"),
        ]
        loadWhenGatewayIsReady(components.url!, attempt: 0)
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
        if message.name == "secondEgoConsole" {
            NSLog("SecondEgo UI JavaScript error: %@", String(describing: message.body))
            return
        }
        guard message.name == "secondEgoWindow", let expanded = message.body as? Bool else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.window.setFrame(self.bounds(expanded: expanded), display: true, animate: true)
            NSApp.activate(ignoringOtherApps: true)
            NSRunningApplication.current.activate(options: [.activateAllWindows, .activateIgnoringOtherApps])
            if expanded {
                self.window.makeKeyAndOrderFront(nil)
                self.webView.window?.makeFirstResponder(self.webView)
            } else {
                self.window.orderFrontRegardless()
            }
        }
    }

    private func loadWhenGatewayIsReady(_ url: URL, attempt: Int) {
        var request = URLRequest(url: URL(string: "http://127.0.0.1:\(port)/api/health")!)
        request.setValue(token, forHTTPHeaderField: "X-SecondEgo-Token")
        URLSession.shared.dataTask(with: request) { [weak self] _, response, _ in
            let ready = (response as? HTTPURLResponse)?.statusCode == 200
            if ready || attempt >= 60 {
                DispatchQueue.main.async {
                    if !ready { NSLog("SecondEgo gateway readiness timed out; loading UI anyway") }
                    self?.webView.load(URLRequest(url: url))
                }
                return
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                self?.loadWhenGatewayIsReady(url, attempt: attempt + 1)
            }
        }.resume()
    }

    func stop() {
        gateway?.terminate()
    }

    private func bounds(expanded: Bool) -> NSRect {
        let screen = NSScreen.main ?? NSScreen.screens[0]
        let screenFrame = screen.frame
        let width: CGFloat = expanded ? 760 : 236
        let height: CGFloat = expanded ? 620 : 38
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
        environment["SECONDEGO_UI_DIST"] = root.appendingPathComponent("apps/desktop/dist").path
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
