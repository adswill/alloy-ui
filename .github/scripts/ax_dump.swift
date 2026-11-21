// Dump the macOS accessibility (AX) tree of a running process and require the counter's buttons.
import ApplicationServices
import Foundation

let pid = pid_t(CommandLine.arguments[1])!
print("AXIsProcessTrusted: \(AXIsProcessTrusted())")
var seen = Set<String>()
func str(_ e: AXUIElement, _ a: String) -> String? {
    var v: CFTypeRef?
    guard AXUIElementCopyAttributeValue(e, a as CFString, &v) == .success else { return nil }
    return v as? String
}
func walk(_ e: AXUIElement, _ depth: Int) {
    let role = str(e, kAXRoleAttribute) ?? "?"
    let title = str(e, kAXTitleAttribute) ?? str(e, kAXDescriptionAttribute) ?? str(e, kAXValueAttribute) ?? ""
    print(String(repeating: "  ", count: depth) + "\(role) '\(title)'")
    seen.insert(title)
    var kids: CFTypeRef?
    if AXUIElementCopyAttributeValue(e, kAXChildrenAttribute as CFString, &kids) == .success,
       let arr = kids as? [AXUIElement], depth < 12 { for k in arr { walk(k, depth + 1) } }
}
let app = AXUIElementCreateApplication(pid)
walk(app, 0)
let missing = ["Add", "Subtract", "Reset"].filter { !seen.contains($0) }
if missing.isEmpty { print("AX OK") } else { print("AX: missing \(missing)"); exit(3) }
