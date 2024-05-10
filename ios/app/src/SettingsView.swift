import SwiftUI

extension String: Identifiable {
    public typealias ID = Int
    public var id: Int {
        return hash
    }
}

class SettingsViewModel: ObservableObject {
    @Published var name: String = ""
    @Published var type: DeviceType = .desktop
    @Published var trustedNetworks: String = ""

    init() {
        try? loadSettings()
    }

    func loadSettings() throws {
        name = try String(contentsOf: URL(fileURLWithPath: "/var/mobile/kdeconnect/name"), encoding: .utf8)
        type = try DeviceType.fromString(try String(contentsOf: URL(fileURLWithPath: "/var/mobile/kdeconnect/type"), encoding: .utf8))
        trustedNetworks = try String(contentsOf: URL(fileURLWithPath: "/var/mobile/kdeconnect/trusted"), encoding: .utf8)
    }

    func saveSettings() throws {
        try name.write(to: URL(fileURLWithPath: "/var/mobile/kdeconnect/name"), atomically: true, encoding: .utf8) 
        try type.toString().lowercased().write(to: URL(fileURLWithPath: "/var/mobile/kdeconnect/type"), atomically: true, encoding: .utf8) 
        try trustedNetworks.write(to: URL(fileURLWithPath: "/var/mobile/kdeconnect/trusted"), atomically: true, encoding: .utf8) 
    }
}

struct SettingsView: View {
    @ObservedObject var state: SettingsViewModel = SettingsViewModel() 
    var exit: () -> Void 

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
        .navigationTitle("Settings")
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
                        exit()
                    } catch {
                        UIApplication.shared.alert(body: error.localizedDescription)
                    }
                }
            }
        }
    }
}
