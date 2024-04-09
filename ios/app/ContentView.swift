import SwiftUI

struct Device: Identifiable {
    var name: String
    let id: String
}

struct ContentView: View {
    @State var connected: [Device] = [];
    init() {
        createMessageCenter()
        self.refreshConnectedDevices()
    }
    func refreshConnectedDevices() { 
        var connectedArr = getConnectedDevices() as! [NSDictionary]
        self.connected = connectedArr.map { 
            var name = $0.value(forKey: "name")! as! String
            var id = $0.value(forKey: "id")! as! String
            var device = Device(name: name, id: id)
            return device 
        }
    }
	var body: some View {
        NavigationView {
            VStack {
                Button("Start daemon (TrollStore only)") {
                    let bundlePath = Bundle.main.bundlePath
                    let daemonPath = "\(bundlePath)/kdeconnectd"
                    spawnRoot(daemonPath, [])
                }
                List(self.$connected) { device in
                    Text(device.name.wrappedValue)
                }.refreshable {
                    self.refreshConnectedDevices()
                }
            }
            .padding()
            .navigationTitle("KDE Connect")
        }
	}
}
