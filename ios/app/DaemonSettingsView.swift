import SwiftUI
import libroot

extension String: Identifiable {
    public typealias ID = Int
    public var id: Int {
        return hash
    }
}

class DaemonSettingsViewModel: ObservableObject {
    @Published var name: String = ""
    @Published var type: DeviceType = .desktop
    @Published var trustedNetworks: String = ""

    init() {
        try? loadSettings()
    }

    func loadSettings() throws {
        name = try String(contentsOf: URL(fileURLWithPath: jbRootPath("/var/mobile/kdeconnect/name")), encoding: .utf8)
        type = try DeviceType.fromString(try String(contentsOf: URL(fileURLWithPath: jbRootPath("/var/mobile/kdeconnect/type")), encoding: .utf8))
        trustedNetworks = try String(contentsOf: URL(fileURLWithPath: jbRootPath("/var/mobile/kdeconnect/trusted")), encoding: .utf8)
    }

    func saveSettings() throws {
        try name.write(to: URL(fileURLWithPath: jbRootPath("/var/mobile/kdeconnect/name")), atomically: true, encoding: .utf8) 
        try type.toString().lowercased().write(to: URL(fileURLWithPath: jbRootPath("/var/mobile/kdeconnect/type")), atomically: true, encoding: .utf8) 
        try trustedNetworks.write(to: URL(fileURLWithPath: jbRootPath("/var/mobile/kdeconnect/trusted")), atomically: true, encoding: .utf8) 
        sendExit()
    }
}

struct DaemonSettingsView: View {
    @ObservedObject var state: DaemonSettingsViewModel = DaemonSettingsViewModel() 
    var body: some View {
        List {
            HStack {
                Text("Name")
                TextField("Name", text: $state.name).multilineTextAlignment(.trailing)
            }
            HStack {
                Picker("Type", selection: $state.type) {
                    ForEach(DeviceType.allCases) { type in
                        Text(type.toString())
                    }
                }
            }
            NavigationLink("Trusted Networks") {
                List {
                    Section("One network per line") {
                        TextEditor(text: $state.trustedNetworks).font(.system(.body, design: .monospaced))
                    }
                }.navigationTitle("Trusted Networks")
            }
        }
        .navigationTitle("Daemon Settings")
        .toolbar {
            ToolbarItemGroup(placement: .navigationBarTrailing) {
                Button("Load") {
                    do {
                        try state.loadSettings()
                    } catch {
                        UIApplication.shared.alert(body: error.localizedDescription)
                    }
                }
                Button("Save") {
                    do {
                        try state.saveSettings()
                    } catch {
                        UIApplication.shared.alert(body: error.localizedDescription)
                    }
                }
            }
        }
    }
}
