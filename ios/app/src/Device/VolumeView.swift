import SwiftUI

struct VolumeStreamView: View {
    @Binding var stream: ConnectedDeviceVolumeStream
    @State var cachedVolume: Int = -1
    var refresh: () -> Void
    var id: String

    var body: some View {
        let enabledProxy = Binding<Bool>(
            get: {stream.enabled ?? false},
            set: {sendVolume(id, stream.id, NSNumber(value: $0), NSNumber(value: stream.muted), NSNumber(value: stream.volume))}
        )
        let mutedProxy = Binding<Bool>(
            get: {stream.muted},
            set: {sendVolume(id, stream.id, NSNumber(value: stream.enabled ?? false), NSNumber(value: $0), NSNumber(value: stream.volume))}
        )
        let volumeProxy = Binding<Float>(
            get: {cachedVolume != -1 ? Float(cachedVolume) : Float(stream.volume)},
            set: {cachedVolume = Int($0)}
        )
        
        NavigationLink("ID") {
            VStack {
                List {
                    Button("Copy") {
                        UIPasteboard.general.string = stream.id
                    }
                    Text(stream.id)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .font(.system(.body, design: .monospaced))
                }
            }
            .navigationTitle("ID")
            .refreshable {
                refresh()
                requestVolume(id)
            }
        }
        if stream.enabled != nil {
            Toggle("Enabled", isOn: enabledProxy)
        }
        Toggle("Muted", isOn: mutedProxy)
        HStack {
            Slider(value: volumeProxy, in: 0...Float(stream.maxVolume), step: 1) {
                Text("Volume")
            } minimumValueLabel: {
                Image(systemName: "speaker.wave.1.fill")
            } maximumValueLabel: {
                Image(systemName: "speaker.wave.3.fill")
            } onEditingChanged: { editing in
                if !editing {
                    sendVolume(id, stream.id, NSNumber(value: stream.enabled ?? false), NSNumber(value: stream.muted), NSNumber(value: cachedVolume))
                    // wait a bit as by then we probably have recieved a response that the volume has changed and it'll be seamless
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                        cachedVolume = -1
                    }
                }
            }.disabled(mutedProxy.wrappedValue).padding(.trailing, 4)

            Text(
                (Float(volumeProxy.wrappedValue)/Float(stream.maxVolume))
                .formatted(.percent.precision(.fractionLength(0)).rounded(rule: .toNearestOrAwayFromZero))
            )
        }
    }
}

struct VolumeView: View {
    @State private var volumeHandler = VolumeButtonHandler()
    @Binding var device: ConnectedDevice
    var refresh: () -> Void

    var body: some View {
        VStack {
            List {
                if device.volume.count > 0 {
                    if let stream = device.volume.filter({ $0.enabled ?? false }).first {
                        Section("Volume Buttons") {
                            Text("Volume buttons are currently controlling: \(stream.description)").padding(.vertical, 4)
                        }
                    }
                    ForEach($device.volume) { $stream in
                        Section(stream.description) {
                            VolumeStreamView(stream: $stream, refresh: { refresh() }, id: device.id)
                        }
                    }
                } else {
                    Text("This device has no volume controls.").padding(.vertical, 4)
                }
            }
        }
        .navigationTitle("Volume")
        .refreshable {
            refresh()
            requestVolume(device.id)
        }
        .onAppear {
            volumeHandler.startHandler(disableSystemVolumeHandler: true)
            volumeHandler.volumeUpPressed = {
                if let stream = device.volume.filter({ $0.enabled ?? false }).first {
                    sendVolume(
                        device.id,
                        stream.id,
                        NSNumber(value: true),
                        NSNumber(value: false),
                        NSNumber(value: min(stream.volume + Int(5 * Float(stream.maxVolume) / 100), stream.maxVolume))
                    )
                }
            }
            volumeHandler.volumeDownPressed = {
                if let stream = device.volume.filter({ $0.enabled ?? false }).first {
                    sendVolume(
                        device.id,
                        stream.id,
                        NSNumber(value: true),
                        NSNumber(value: false),
                        NSNumber(value: max(stream.volume - Int(5 * Float(stream.maxVolume) / 100), 0))
                    )
                }
            }
        }
        .onDisappear {
            volumeHandler.stopHandler()
        }
    }
}
