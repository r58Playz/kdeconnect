import SwiftUI

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
