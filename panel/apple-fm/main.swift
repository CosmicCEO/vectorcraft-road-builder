// apple-fm-agent: a small bridge from the Road Builder panel to Apple's on-device model (Foundation Models, Apple Intelligence).
//   apple-fm-agent --check                      prints whether the model is available, as JSON
//   echo '{"system": "...", "user": "..."}' | apple-fm-agent     prints the model's answer (plain text)
// Used as a `cli` provider in panel/providers.json. Nothing leaves the Mac.
import Foundation
import FoundationModels

@main
struct AppleFMAgent {
    static func availabilityJSON() -> (ok: Bool, text: String) {
        switch SystemLanguageModel.default.availability {
        case .available:
            return (true, #"{"available": true}"#)
        case .unavailable(let reason):
            return (false, #"{"available": false, "reason": "\#(reason)"}"#)
        }
    }

    static func main() async {
        if CommandLine.arguments.contains("--check") {
            let a = availabilityJSON()
            print(a.text)
            exit(a.ok ? 0 : 3)
        }
        let input = FileHandle.standardInput.readDataToEndOfFile()
        guard let obj = try? JSONSerialization.jsonObject(with: input) as? [String: Any],
              let user = obj["user"] as? String else {
            FileHandle.standardError.write(Data("expected JSON on stdin with a \"user\" field\n".utf8))
            exit(2)
        }
        let a = availabilityJSON()
        guard a.ok else {
            FileHandle.standardError.write(Data("Apple Intelligence is not available: \(a.text)\n".utf8))
            exit(3)
        }
        do {
            let session = LanguageModelSession(instructions: (obj["system"] as? String) ?? "")
            let options = GenerationOptions(temperature: 0)
            let response = try await session.respond(to: user, options: options)
            print(response.content)
        } catch {
            FileHandle.standardError.write(Data("Apple model error: \(error)\n".utf8))
            exit(1)
        }
    }
}
