import SwiftUI

extension String {
    func deletingPrefix(_ prefix: String) -> String {
        guard self.hasPrefix(prefix) else { return self }
        return String(self.dropFirst(prefix.count))
    }
}

struct ShareView: View {
    @State var filePickerShowing = false
    @State var openFileOnShare = false
    @Binding var device: ConnectedDevice

    var body: some View {
        VStack {
            List {
                Section(header: Text("Files")) {
                    Button("Share file") {
                        filePickerShowing.toggle()
                    }
                    Toggle("Open once sent", isOn: $openFileOnShare)
                }.fileImporter(isPresented: $filePickerShowing, allowedContentTypes: [.item], allowsMultipleSelection: true) {
                    if let urls = try? $0.get() {
                        sendFiles(device.id, urls.map({ $0.absoluteString.deletingPrefix("file://") }), NSNumber(value: openFileOnShare))
                    }
                }
            }
        }
        .navigationTitle("Share")
    }
}
