import SwiftUI

struct CommandsView: View {
    @Binding var device: ConnectedDevice 
    var refresh: () -> Void
    var body: some View {
        VStack {
            List {
                if (device.command.count > 0) {
                    ForEach(device.command) { command in
                        Button {
                            runCommand(device.id, command.id)
                        } label: {
                            VStack {
                                Text(command.name).lineLimit(1).truncationMode(.tail)
                                Text(command.command).font(.system(.caption, design: .monospaced)).lineLimit(1).truncationMode(.tail)
                            }
                        }.foregroundColor(.uikitLabel)
                    }
                } else {
                    Text("This device has no commands.").padding(.vertical, 4)
                }
            }
        }
        .navigationTitle("Commands")
        .refreshable {
            refresh()
            requestCommands(device.id)
        }
    }
}
