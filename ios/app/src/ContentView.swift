import SwiftUI
import CoreMotion

var motionManager: CMMotionManager = CMMotionManager()


enum DeviceType: Int, CaseIterable, Identifiable {
    case desktop = 0
    case laptop = 1
    case phone = 2
    case tablet = 3
    case tv = 4

    var id: Self { self }

    static func fromString(_ str: String) throws -> Self {
        switch str {
            case "desktop":
                return .desktop
            case "laptop":
                return .laptop
            case "phone":
                return .phone
            case "tablet":
                return .tablet
            case "tv":
                return .tv
            default:
                throw "invalid"
        }
    }

    func toString() -> String {
        switch self {
            case .desktop:
                return "Desktop"
            case .laptop:
                return "Laptop"
            case .phone:
                return "Phone"
            case .tablet:
                return "Tablet"
            case .tv:
                return "TV"
        }
    }

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

struct PairedDevice: Identifiable, Equatable {
    var name: String
    var id: String
    var type: DeviceType 
}

struct ConnectedDevice: Identifiable, Equatable {
    var name: String
    var id: String
    var type: DeviceType
    var paired: Bool
    var batteryLevel: Int
    var batteryCharging: Bool
    var batteryLow: Bool
    var clipboard: String
}

func batteryToSFSymbol(device: Binding<ConnectedDevice>) -> String {
    if device.batteryLevel.wrappedValue > 75 {
        return "battery.100"
    } else if device.batteryLevel.wrappedValue > 50 {
        return "battery.75"
    } else if device.batteryLevel.wrappedValue > 25 {
        return "battery.50"
    } else if device.batteryLevel.wrappedValue > 0 {
        return "battery.25"
    } else {
        return "battery.0"
    }
}

@objc public class KConnectSwiftServer: NSObject {
    var view: ContentView
    init(view: ContentView) {
        self.view = view
    }

    @objc func refreshRequested() {
        self.view.refreshDevicesViews()
    }
}

class ContentViewModel: ObservableObject {
    @Published var connected = [ConnectedDevice]()
    @Published var paired = [PairedDevice]()
}

struct ContentView: View {
    @ObservedObject var data = ContentViewModel()
    @State var server: KConnectObjcServer! = nil

    init() {
        self.server = KConnectObjcServer.new(withSwift: KConnectSwiftServer(view: self));
        createMessageCenter()
        let proc = sysctl_ps() as! [NSDictionary]
        let kdeconnectd = proc.first { $0.value(forKey: "proc_name") as! String == "kdeconnectd" }
        if kdeconnectd != nil {
            do {
                try self.refreshConnectedDevices(e:false)
                try self.refreshPairedDevices()
            } catch {
                // ignore
            }
        }
    }

    func refreshConnectedDevices(e:Bool) throws {
        guard let connectedArr = getConnectedDevices() as? [NSDictionary] else {
            throw "Error getting connected devices"
        }
        let connectedMapped = try connectedArr.map {
            if let name = $0.value(forKey: "name") as? String,
                let id = $0.value(forKey: "id") as? String,
                let type = $0.value(forKey: "type") as? Int,
                let paired = $0.value(forKey: "paired") as? Int,
                let batteryLevel = $0.value(forKey: "battery_level") as? Int,
                let batteryCharging = $0.value(forKey: "battery_charging") as? Int,
                let batteryUnderThreshold = $0.value(forKey: "battery_under_threshold") as? Int,
                let clipboard = $0.value(forKey: "clipboard") as? String,
                let parsedType = DeviceType(rawValue: type) {
                    let device = ConnectedDevice(
                        name: name,
                        id: id,
                        type: parsedType,
                        paired: paired == 1,
                        batteryLevel: batteryLevel,
                        batteryCharging: batteryCharging == 1,
                        batteryLow: batteryUnderThreshold == 1,
                        clipboard: clipboard
                    )
                    return device
            } else {
                throw "Error parsing connected devices"
            }
        }
        self.data.connected = connectedMapped
    }

    func refreshPairedDevices() throws {
        guard let pairedArr = getPairedDevices() as? [NSDictionary] else {
            throw "Error getting paired devices"
        }
        let pairedMapped = try pairedArr.map {
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
        let pairedFiltered = pairedMapped.filter { el in
            !data.connected.contains(where: { $0.id == el.id })
        }
        self.data.paired = pairedFiltered
    }

    func refreshDevicesViews() {
        let proc = sysctl_ps() as! [NSDictionary]
        let kdeconnectd = proc.first { $0.value(forKey: "proc_name") as! String == "kdeconnectd" }
        if kdeconnectd != nil {
            do {
                rebroadcast()
                try self.refreshConnectedDevices(e:true)
                try self.refreshPairedDevices()
            } catch {
                UIApplication.shared.alert(body: error.localizedDescription)
            }
        }
    }

	var body: some View {
        NavigationView {
            VStack {
                List {
                    Section(header: Text("Connected devices")) {
                        ForEach(self.$data.connected, id: \.id) { $device in
                            NavigationLink {
                                ConnectedDeviceView(device: $device, refresh: { refreshDevicesViews() })
                            } label: {
                                HStack {
                                    Image(systemName: device.type.toSFSymbol())
                                    if device.paired {
                                        if device.batteryCharging {
                                            Image(systemName: "battery.100.bolt").foregroundStyle(.green)
                                        } else if device.batteryLow {
                                            Image(systemName: batteryToSFSymbol(device: $device)).foregroundStyle(.red)
                                        } else {
                                            Image(systemName: batteryToSFSymbol(device: $device))
                                        }
                                        Text(device.batteryLevel, format: .percent)
                                    }
                                    VStack(alignment: .leading) {
                                        Text(device.name).lineLimit(1).truncationMode(.tail)
                                        Text(device.id).font(.caption).lineLimit(1).truncationMode(.tail)
                                    }
                                }
                            }
                        }
                    }
                    Section(header: Text("Paired devices")) {
                        ForEach(self.$data.paired, id: \.id) { $device in
                            HStack {
                                Image(systemName: device.type.toSFSymbol())
                                VStack(alignment: .leading) {
                                    Text(device.name).lineLimit(1).truncationMode(.tail)
                                    Text(device.id).font(.caption).lineLimit(1).truncationMode(.tail)
                                }
                            }
                        }
                    }
                    Section(header: Text("Tools")) {
                        NavigationLink("Settings") {
                            SettingsView()
                        }
                        if let resourceURL = Bundle.main.resourceURL,
                            FileManager().fileExists(atPath:  resourceURL.deletingLastPathComponent().appendingPathComponent("_TrollStore").path) {
                            Button("Start daemon") {
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
                            Button("Kill daemon") {
                                sendExit()
                            }
                        } else {
                            Button("Restart daemon") {
                                sendExit()
                            }
                        }
                    }
                    Section(header: Text("Info")) {
                        VStack(alignment: .leading) {
                            Text("kdeconnectjb").font(.system(.title, design: .monospaced)).frame(maxWidth: .infinity, alignment: .center).padding(.bottom, 8)
                            Text(
                                """
                                KDE Connect implementation in Rust and application for jailbroken/TrollStore iOS.

                                This application is built in SwiftUI with Objective-C glue for communicating with the daemon.
                                The daemon is built in Rust and Objective-C, with a little Swift.
                                """)
                                .multilineTextAlignment(.leading)
                        }
                    }
                    Section(header: Text("Credits")) {
                        VStack(alignment: .leading) {
                            Link("r58Playz", destination: URL(string: "https://github.com/r58Playz")!)
                                .font(.system(.title2, design: .monospaced))
                                .padding(.bottom, 4)
                            Text("Made KDE Connect implementation in Rust, daemon, and parts of the UI.")
                        }
                        VStack(alignment: .leading) {
                            Link("BomberFish", destination: URL(string: "https://github.com/BomberFish")!)
                                .font(.system(.title2, design: .monospaced))
                                .padding(.bottom, 4)
                            Text("Helped with various features and the UI, especially dealing with SwiftUI.")
                        }
                        VStack(alignment: .leading) {
                            Link("KDE Connect Team", destination: URL(string: "https://invent.kde.org/network/kdeconnect-meta/-/blob/master/README.md")!)
                                .font(.system(.title2, design: .monospaced))
                                .padding(.bottom, 4)
                            Text("Without their effort, the KDE Connect protocol and apps would not exist.")
                        }
                    }
                }
                .listStyle(InsetGroupedListStyle())
                .refreshable {
                    refreshDevicesViews()
                }
            }
            .navigationTitle("KDE Connect")
        }
        .navigationViewStyle(.stack)
	}
}

struct ConnectedDeviceView: View {
    var device: Binding<ConnectedDevice>
    var refresh: () -> Void

    @ViewBuilder var actions: some View {
        Section(header: Text("Actions")) {
            Button(device.paired.wrappedValue ? "Unpair" : "Pair") {
                sendPairReq(device.id.wrappedValue, device.paired.wrappedValue ? 0 : 1)
            }
            Button("Send ping") {
                sendPing(device.id.wrappedValue)
            }
            Button("Find") {
                sendFind(device.id.wrappedValue)
            }
            NavigationLink("Presenter") {
                PresenterView(device: device)
            }
        }
    }

    @ViewBuilder var info: some View {
        Section(header: Text("Information")) {
            HStack {
                Text("Name")
                Spacer()
                Text(device.name.wrappedValue)
            }
            HStack {
                Text("ID")
                Spacer()
                Text(device.id)
            }
            HStack {
                Text("Type")
                Spacer()
                Text(device.type.wrappedValue.toString())
            }
            HStack {
                Text("Paired")
                Spacer()
                Text(device.paired.wrappedValue ? "Yes" : "No")
            }
        }
    }

    @ViewBuilder var state: some View {
        Section(header: Text("State")) {
            HStack {
                Text("Battery level")
                Spacer()
                Text("\(device.batteryLevel.wrappedValue)")
            }
            HStack {
                Text("Battery charging")
                Spacer()
                Text(device.batteryCharging.wrappedValue ? "Yes" : "No")
            }
            HStack {
                Text("Battery low")
                Spacer()
                Text(device.batteryLow.wrappedValue ? "Yes" : "No")
            }
            NavigationLink("Clipboard") {
                ClipboardView(device: device, refresh: { refresh() })
            }
        }
    }

    var body: some View {
        VStack {
            List {
                actions
                info
                state
            }
            .listStyle(InsetGroupedListStyle())
            .refreshable {
                refresh()
            }
        }
        .navigationTitle(device.name.wrappedValue)
    }
}

struct ClipboardView: View {
    var device: Binding<ConnectedDevice>
    var refresh: () -> Void
    var body: some View {
        VStack {
            List {
                Button("Copy") {
                    UIPasteboard.general.string = device.clipboard.wrappedValue
                }
                Text(device.clipboard.wrappedValue)
                    .multilineTextAlignment(.leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .font(.system(.body, design: .monospaced))
            }
        }
        .navigationTitle("Clipboard")
        .refreshable {
            refresh()
        }
    }
}
