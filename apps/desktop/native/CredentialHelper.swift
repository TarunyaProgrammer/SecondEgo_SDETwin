import Foundation
import Security

// Minimal macOS Keychain bridge for Electron's main process.
// Commands: save|read|delete SERVICE ACCOUNT
// Secrets are read from stdin and never appear in process arguments.

enum HelperFailure: Error {
    case usage
    case keychain(OSStatus)
    case missing
}

func fail(_ error: HelperFailure) -> Never {
    let message: String
    switch error {
    case .usage:
        message = "invalid credential helper request"
    case .missing:
        message = "credential not found"
    case .keychain(let status):
        message = "keychain operation failed (\(status))"
    }
    FileHandle.standardError.write(Data("\(message)\n".utf8))
    exit(1)
}

let arguments = CommandLine.arguments
guard arguments.count == 4 else { fail(.usage) }
let command = arguments[1]
let service = arguments[2]
let account = arguments[3]
guard !service.isEmpty, !account.isEmpty else { fail(.usage) }

let baseQuery: [CFString: Any] = [
    kSecClass: kSecClassGenericPassword,
    kSecAttrService: service,
    kSecAttrAccount: account,
]

func readStdin() -> Data {
    var data = FileHandle.standardInput.readDataToEndOfFile()
    // Electron appends one line ending when saving. Remove only that transport
    // delimiter; API key whitespace inside the value is preserved.
    if data.last == 10 {
        data.removeLast()
        if data.last == 13 { data.removeLast() }
    }
    return data
}

switch command {
case "save":
    var attributes = baseQuery
    attributes[kSecValueData] = readStdin()
    let existingStatus = SecItemCopyMatching(baseQuery as CFDictionary, nil)
    let status: OSStatus
    if existingStatus == errSecSuccess {
        status = SecItemUpdate(baseQuery as CFDictionary, [kSecValueData: attributes[kSecValueData]!] as CFDictionary)
    } else if existingStatus == errSecItemNotFound {
        status = SecItemAdd(attributes as CFDictionary, nil)
    } else {
        fail(.keychain(existingStatus))
    }
    guard status == errSecSuccess else { fail(.keychain(status)) }
case "read":
    var query = baseQuery
    query[kSecReturnData] = true
    query[kSecMatchLimit] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    guard status == errSecSuccess else {
        if status == errSecItemNotFound { fail(.missing) }
        fail(.keychain(status))
    }
    guard let data = result as? Data else { fail(.keychain(errSecDecode)) }
    FileHandle.standardOutput.write(data)
case "delete":
    let status = SecItemDelete(baseQuery as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else { fail(.keychain(status)) }
default:
    fail(.usage)
}
