import SwiftUI

struct Device: Identifiable {
    var name: String
    let id: String
}

struct ContentView: View {
    @State var connected: [Device] = [];
    init() {
        createMessageCenter()
        let proc = sysctl_ps() as! [NSDictionary]
        let kdeconnectd = proc.first { $0.value(forKey: "proc_name") as! String == "kdeconnectd" }
        if kdeconnectd != nil {
            do {
                try self.refreshConnectedDevices()
            } catch {
                UIApplication.shared.alert(body: error.localizedDescription)
            }
        }
    }
    func refreshConnectedDevices() throws {
        guard let connectedArr = getConnectedDevices() as? [NSDictionary] else {
            throw "Error getting connected devices"
        }
        self.connected = try connectedArr.map {
            if let name = $0.value(forKey: "name") as? String,
               let id = $0.value(forKey: "id") as? String {
                let device = Device(name: name, id: id)
                return device
            } else {
             throw "Error parsing connected devices"
            }
        }
    }
	var body: some View {
        NavigationView {
            VStack {
                Button("Start daemon (TrollStore only)") { // TODO: Detect if installed via TrollStore
                    let bundlePath = Bundle.main.bundlePath
                    let daemonPath = "\(bundlePath)/kdeconnectd"
                    spawnRoot(daemonPath, [])
                }
                List(self.$connected) { device in
                    Text(device.name.wrappedValue)
                }.refreshable {
                    do {
                        try self.refreshConnectedDevices()
                    } catch {
                        UIApplication.shared.alert(body: error.localizedDescription)
                    }
                }
            }
            .padding()
            .navigationTitle("KDE Connect")
        }
	}
}
