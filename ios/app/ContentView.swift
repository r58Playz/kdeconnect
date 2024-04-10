import SwiftUI

enum DeviceType: Int{
    case desktop = 0
    case laptop = 1
    case phone = 2
    case tablet = 3
    case tv = 4

    func toSFSymbol() -> String {
        switch self {
            case .desktop:
                return "desktopcomputer"
            case .laptop:
                return "laptopcomputer"
            case .phone:
                return "iphone.gen1"
            case .tablet:
                return "ipad.gen1"
            case .tv:
                return "tv"
        }
    }
}

struct PairedDevice: Identifiable {
    var name: String
    var id: String
    var type: DeviceType 
}

struct ConnectedDevice: Identifiable {
    var name: String
    var id: String
    var type: DeviceType
    var batteryLevel: Int
    var batteryCharging: Bool
    var batteryLow: Bool
}

func batteryToSFSymbol(device: Binding<ConnectedDevice>) -> String {
    if device.batteryLevel.wrappedValue > 75 {
        return "battery.100percent"
    } else if device.batteryLevel.wrappedValue > 50 {
        return "battery.75percent"
    } else if device.batteryLevel.wrappedValue > 25 {
        return "battery.50percent"
    } else if device.batteryLevel.wrappedValue > 0 {
        return "battery.25percent"
    } else {
        return "battery.0percent"
    }
}

struct ContentView: View {
    @State var connected: [ConnectedDevice] = []
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
                let id = $0.value(forKey: "id") as? String,
                let type = $0.value(forKey: "type") as? Int,
                let batteryLevel = $0.value(forKey: "battery_level") as? Int,
                let batteryCharging = $0.value(forKey: "battery_charging") as? Int,
                let batteryUnderThreshold = $0.value(forKey: "battery_under_threshold") as? Int,
                let parsedType = DeviceType(rawValue: type) {
                    let device = ConnectedDevice(
                        name: name,
                        id: id,
                        type: parsedType,
                        batteryLevel: batteryLevel,
                        batteryCharging: batteryCharging == 1,
                        batteryLow: batteryUnderThreshold == 1
                    )
                    return device
            } else {
                throw "Error parsing connected devices"
            }
        }
    }

    func refreshPairedDevices() throws {
        guard let paired = getPairedDevices() as? [NSDictionary] else {
            throw "Error getting paired devices"
        }
        let pairedMapped = try paired.map {
            if let name = $0.value(forKey: "name") as? String,
                let id = $0.value(forKey: "id") as? String,
                let type = $0.value(forKey: "type") as? Int,
                let parsedType = DeviceType(rawValue: type) {
                    let device = PairedDevice(
                        name: name,
                        id: id,
                        type: parsedType
                    )
                    return device
            } else {
                throw "Error parsing paired devices"
            }
        }
    }
	var body: some View {
        NavigationView {
            VStack {
                List {
                    Section(header: Text("Connected devices")) {
                        ForEach(self.$connected) { device in
                            HStack {
                                Image(systemName: device.type.wrappedValue.toSFSymbol())
                                // this doesn't work??
                                if device.batteryCharging.wrappedValue {
                                    Image(systemName: "battery.100percent.bolt").foregroundStyle(.green)
                                } else if device.batteryLow.wrappedValue {
                                    Image(systemName: batteryToSFSymbol(device: device)).foregroundStyle(.red)
                                } else {
                                    Image(systemName: batteryToSFSymbol(device: device))
                                }
                                VStack {
                                    Text(device.name.wrappedValue)
                                }
                            }
                        }
                    }
                    Section(header: Text("Paired devices")) {

                    }
                    Section(header: Text("Settings")) {
                        Button("Start daemon (TrollStore only)") { // TODO: Detect if installed via TrollStore
                            let proc = sysctl_ps() as! [NSDictionary]
                            let kdeconnectd = proc.first { $0.value(forKey: "proc_name") as! String == "kdeconnectd" }
                            if kdeconnectd == nil {
                                let bundlePath = Bundle.main.bundlePath
                                let daemonPath = "\(bundlePath)/kdeconnectd"
                                if !FileManager.default.fileExists(atPath: daemonPath) {
                                    UIApplication.shared.alert(body: "Daemon not found")
                                    return
                                }
                                let ret = spawnRoot(daemonPath, [])
                                if ret != 0 {
                                    UIApplication.shared.alert(body: "Error starting daemon: \(ret)")
                                    return
                                }
                            }
                        }
                    }
                }
                .listStyle(InsetGroupedListStyle())
                .refreshable {
                    do {
                        try self.refreshConnectedDevices()
                    } catch {
                        UIApplication.shared.alert(body: error.localizedDescription)
                    }
                }
            }
            .navigationTitle("KDE Connect")
        }
	}
}
