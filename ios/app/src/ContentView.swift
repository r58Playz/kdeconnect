import SwiftUI
import CoreMotion

// Compatibility with catppuccin-ios
extension ShapeStyle where Self == Color {
    static var uikitGreen: Color {
        Color(UIColor.systemGreen)
    }

    static var uikitRed: Color {
        Color(UIColor.systemRed)
    }

    static var uikitSecondarySystemBackground: Color {
        Color(UIColor.secondarySystemBackground)
    }

    static var uikitLabel: Color {
        Color(UIColor.label)
    }

    static var uikitSecondaryLabel: Color {
        Color(UIColor.secondaryLabel)
    }

    static var uikitTertiaryLabel: Color {
        Color(UIColor.tertiaryLabel)
    }
}

var motionManager: CMMotionManager = CMMotionManager()
var settingsModel: SettingsViewModel = SettingsViewModel()

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

struct ConnectedDeviceConnectivitySignal: Identifiable, Equatable {
    var id: String
    var type: String
    var strength: Int
}

struct ConnectedDeviceVolumeStream: Identifiable, Equatable {
    var id: String
    var description: String
    var enabled: Bool?
    var muted: Bool
    var maxVolume: Int
    var volume: Int
}

enum ConnectedDeviceMprisPlayerLoop: Int, Identifiable, Equatable {
    case None = 0
    case Track = 1
    case Playlist = 2

    var id: Self { self }
}

struct ConnectedDeviceMprisPlayer: Identifiable, Equatable, Hashable {
    var id: String

    var title: String
    var artist: String
    var album: String

    var albumArt: String

    var url: String

    var isPlaying: Bool 

    var canPause: Bool 
    var canPlay: Bool 

    var canGoNext: Bool 
    var canGoPrevious: Bool 

    var canSeek: Bool 

    var shuffle: Bool 

    var position: Int
    var length: Int
    var volume: Int

    var loopStatus: ConnectedDeviceMprisPlayerLoop
}

struct ConnectedDeviceCommand: Identifiable, Equatable, Hashable {
    var id: String
    var name: String
    var command: String
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
    var connectivity: [ConnectedDeviceConnectivitySignal]
    var volume: [ConnectedDeviceVolumeStream]
    var player: [ConnectedDeviceMprisPlayer]
    var command: [ConnectedDeviceCommand]
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
        do {
            try self.refreshConnectedDevices()
            try self.refreshPairedDevices()
        } catch {
            // ignore
        }
    }

    func parseConnectivity(dict: NSDictionary) throws -> ConnectedDeviceConnectivitySignal {
        if let id = dict.value(forKey: "id") as? String,
            let type = dict.value(forKey: "type") as? String,
            let signal = dict.value(forKey: "signal") as? Int {
            return ConnectedDeviceConnectivitySignal(id: id, type: type, strength: signal)
        } else {
            throw "Error parsing connected device connectivity"
        }
    }

    func parseVolume(dict: NSDictionary) throws -> ConnectedDeviceVolumeStream {
        if let name = dict.value(forKey: "name") as? String,
            let description = dict.value(forKey: "description") as? String,
            let hasEnabled = dict.value(forKey: "has_enabled") as? Bool,
            let enabled = dict.value(forKey: "enabled") as? Bool,
            let muted = dict.value(forKey: "muted") as? Bool,
            let maxVolume = dict.value(forKey: "max_volume") as? Int,
            let volume = dict.value(forKey: "volume") as? Int {
            return ConnectedDeviceVolumeStream(
                id: name,
                description: description,
                enabled: hasEnabled ? enabled : nil,
                muted: muted,
                maxVolume: maxVolume,
                volume: volume
            )
        } else {
            throw "Error parsing connected device volume"
        }
    }

    func parsePlayer(dict: NSDictionary) throws -> ConnectedDeviceMprisPlayer {
        if let id = dict.value(forKey: "id") as? String,
            let title = dict.value(forKey: "title") as? String,
            let artist = dict.value(forKey: "artist") as? String,
            let album = dict.value(forKey: "album") as? String,
            let albumArt = dict.value(forKey: "album_art") as? String,
            let url = dict.value(forKey: "url") as? String,
            let isPlaying = dict.value(forKey: "is_playing") as? Bool,
            let canPause = dict.value(forKey: "can_pause") as? Bool,
            let canPlay = dict.value(forKey: "can_play") as? Bool,
            let canGoNext = dict.value(forKey: "can_go_next") as? Bool,
            let canGoPrevious = dict.value(forKey: "can_go_previous") as? Bool,
            let canSeek = dict.value(forKey: "can_seek") as? Bool,
            let shuffle = dict.value(forKey: "shuffle") as? Bool,
            let position = dict.value(forKey: "position") as? Int,
            let length = dict.value(forKey: "length") as? Int,
            let volume = dict.value(forKey: "volume") as? Int,
            let loop = dict.value(forKey: "loop") as? Int,
            let loopStatus = ConnectedDeviceMprisPlayerLoop(rawValue: loop) {
            return ConnectedDeviceMprisPlayer(
                id: id,
                title: title,
                artist: artist,
                album: album,
                albumArt: albumArt,
                url: url,
                isPlaying: isPlaying,
                canPause: canPause,
                canPlay: canPlay,
                canGoNext: canGoNext,
                canGoPrevious: canGoPrevious,
                canSeek: canSeek,
                shuffle: shuffle,
                position: position,
                length: length,
                volume: volume,
                loopStatus: loopStatus
            ) 
        } else {
            throw "Error parsing connected device mpris player"
        }
    }

    func parseCommand(dict: NSDictionary) throws -> ConnectedDeviceCommand {
        if let id = dict.value(forKey: "id") as? String,
            let name = dict.value(forKey: "name") as? String,
            let command = dict.value(forKey: "command") as? String {
            return ConnectedDeviceCommand(id: id, name: name, command: command)
        } else {
            throw "Error parsing connected device connectivity"
        }
    }

    func refreshConnectedDevices() throws {
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
                let connectivity = $0.value(forKey: "connectivity") as? [NSDictionary],
                let volume = $0.value(forKey: "volume") as? [NSDictionary],
                let player = $0.value(forKey: "player") as? [NSDictionary],
                let command = $0.value(forKey: "command") as? [NSDictionary],
                let parsedType = DeviceType(rawValue: type),
                let parsedConnectivity = try? connectivity.map({ try parseConnectivity(dict: $0) }),
                let parsedVolume = try? volume.map({ try parseVolume(dict: $0) }),
                let parsedPlayer = try? player.map({ try parsePlayer(dict: $0) }),
                let parsedCommand = try? command.map({ try parseCommand(dict: $0) }) {
                    let device = ConnectedDevice(
                        name: name,
                        id: id,
                        type: parsedType,
                        paired: paired == 1,
                        batteryLevel: batteryLevel,
                        batteryCharging: batteryCharging == 1,
                        batteryLow: batteryUnderThreshold == 1,
                        clipboard: clipboard,
                        connectivity: parsedConnectivity,
                        volume: parsedVolume,
                        player: parsedPlayer,
                        command: parsedCommand
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
        do {
            rebroadcast()
            try self.refreshConnectedDevices()
            try self.refreshPairedDevices()
        } catch {
            UIApplication.shared.alert(body: error.localizedDescription)
        }
    }

    func exitDaemon() {
        sendExit()
        self.data.connected = []
        self.data.paired = []
    }

	var body: some View {
        NavigationView {
            VStack {
                List {
                    Section(header: Text("Connected devices")) {
                        if (self.$data.connected.count > 0) {
                            ForEach(self.$data.connected, id: \.id) { $device in
                                NavigationLink {
                                    ConnectedDeviceView(device: $device, refresh: { refreshDevicesViews() })
                                } label: {
                                    HStack {
                                        Image(systemName: device.type.toSFSymbol())
                                        if device.paired {
                                            if device.batteryCharging {
                                                Image(systemName: "battery.100.bolt").foregroundStyle(.uikitGreen)
                                            } else if device.batteryLow {
                                                Image(systemName: batteryToSFSymbol(device: $device)).foregroundStyle(.uikitRed)
                                            } else {
                                                Image(systemName: batteryToSFSymbol(device: $device))
                                            }
                                            Text(device.batteryLevel, format: .percent)
                                        }
                                        VStack(alignment: .leading) {
                                            Text(device.name).lineLimit(1).truncationMode(.tail)
                                            Text(device.id).font(.system(.caption, design: .monospaced)).lineLimit(1).truncationMode(.tail)
                                        }
                                    }
                                }
                            }
                        } else {
                            Text("Connected devices will appear here.").padding(.vertical, 4)
                        }
                    }
                    Section(header: Text("Paired devices")) {
                        if (self.$data.paired.count > 0) {
                            ForEach(self.$data.paired, id: \.id) { $device in
                                HStack {
                                    Image(systemName: device.type.toSFSymbol())
                                    VStack(alignment: .leading) {
                                        Text(device.name).lineLimit(1).truncationMode(.tail)
                                        Text(device.id).font(.system(.caption, design: .monospaced)).lineLimit(1).truncationMode(.tail)
                                    }
                                }
                            }
                        } else {
                            Text("Paired devices that are disconnected will appear here.").padding(.vertical, 4)
                        }
                    }
                    Section(header: Text("Tools")) {
                        NavigationLink("Settings") {
                            SettingsView(state: settingsModel, exit: { exitDaemon() })
                        }
                        if let resourceURL = Bundle.main.resourceURL,
                            FileManager().fileExists(atPath:  resourceURL.deletingLastPathComponent().appendingPathComponent("_TrollStore").path) {
                            Button("Start daemon") {
                                let bundlePath = Bundle.main.bundlePath
                                let daemonPath = "\(bundlePath)/kdeconnectd"
                                if !FileManager.default.fileExists(atPath: daemonPath) {
                                    UIApplication.shared.alert(body: "Daemon not found")
                                    return
                                }
                                let ret = spawn(daemonPath, [])
                                if ret != 0 {
                                    UIApplication.shared.alert(body: "Error starting daemon: \(ret)")
                                    return
                                }
                            }
                            Button("Kill daemon") {
                                exitDaemon()
                            }
                        } else {
                            Button("Restart daemon") {
                                exitDaemon()
                            }
                        }
                        Text("The daemon logs to system logs. Check system logs if you are having any issues.")
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
                        Link("Code", destination: URL(string: "https://github.com/r58Playz/kdeconnect")!)
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
                        VStack(alignment: .leading) {
                            Link("Theos Team", destination: URL(string: "https://theos.dev")!)
                                .font(.system(.title2, design: .monospaced))
                                .padding(.bottom, 4)
                            Text(
                                """
                                This project uses the Theos toolchain and build system extensively. \
                                Also, members in the Theos Discord helped reverse-engineer multiple private APIs used in this project.
                                """
                            )
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
