// Prints the on-screen windows of an app as JSON: swift windows.swift Safari
import CoreGraphics
import Foundation

let owner = CommandLine.arguments.dropFirst().first ?? "Safari"
let list = (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]]) ?? []
let rows: [[String: Any]] = list.compactMap { w in
    guard (w[kCGWindowOwnerName as String] as? String) == owner, (w[kCGWindowLayer as String] as? Int) == 0,
          let b = w[kCGWindowBounds as String] as? [String: CGFloat], (b["Width"] ?? 0) > 200 else { return nil }
    return ["id": w[kCGWindowNumber as String] ?? 0, "name": w[kCGWindowName as String] ?? "", "x": b["X"] ?? 0, "y": b["Y"] ?? 0, "w": b["Width"] ?? 0, "h": b["Height"] ?? 0]
}
print(String(data: try! JSONSerialization.data(withJSONObject: rows), encoding: .utf8)!)
