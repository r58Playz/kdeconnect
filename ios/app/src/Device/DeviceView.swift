import SwiftUI

struct ConnectedDeviceView: View {
    var device: Binding<ConnectedDevice>
    var refresh: () -> Void

    @ViewBuilder var pairedActions: some View {
        Button("Send ping") {
            sendPing(device.id.wrappedValue)
        }
        Button("Find") {
            sendFind(device.id.wrappedValue)
        }
        NavigationLink("Presenter") {
            PresenterView(device: device)
        }
        NavigationLink("Volume controls") {
            VolumeView(device: device, refresh: { refresh() })
        }
        NavigationLink("Share") {
            ShareView(device: device)
        }
        NavigationLink("Media") {
            MprisPlayersView(device: device, refresh: { refresh() })
        }
    }

    @ViewBuilder var actions: some View {
        Section(header: Text("Actions")) {
            Button(device.paired.wrappedValue ? "Unpair" : "Pair") {
                sendPairReq(device.id.wrappedValue, device.paired.wrappedValue ? 0 : 1)
            }
            if (device.paired.wrappedValue) {
                pairedActions
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
            NavigationLink("ID") {
                VStack {
                    List {
                        Button("Copy") {
                            UIPasteboard.general.string = device.id.wrappedValue 
                        }
                        Text(device.id.wrappedValue)
                            .multilineTextAlignment(.leading)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .font(.system(.body, design: .monospaced))
                    }
                }
                .navigationTitle("ID")
                .refreshable {
                    refresh()
                }
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
            NavigationLink("Connectivity report") {
                ConnectivityView(device: device, refresh: { refresh() })
            }
        }
    }

    var body: some View {
        VStack {
            List {
                actions
                info
                if (device.paired.wrappedValue) {
                    state
                }
                Section(header: Text("Internal")) {
                    NavigationLink("Internal representation") {
                        VStack {
                            List {
                                Button("Copy") {
                                    UIPasteboard.general.string = String(reflecting: device.wrappedValue) 
                                }
                                Text(String(reflecting: device.wrappedValue))
                                    .multilineTextAlignment(.leading)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .font(.system(.body, design: .monospaced))
                            }
                        }
                        .navigationTitle("Internal")
                        .refreshable {
                            refresh()
                        }
                    }
                }
            }
            .listStyle(InsetGroupedListStyle())
            .refreshable {
                refresh()
            }
        }
        .navigationTitle(device.name.wrappedValue)
        .onAppear {
            requestVolume(device.id.wrappedValue)
            requestPlayers(device.id.wrappedValue)
        }
    }
}
