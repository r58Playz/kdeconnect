import SwiftUI

struct ContentView: View {
    @State var logs: String = "";
	var body: some View {
        VStack {
            Text("KDE Connect for Jailbreak")
            Spacer()
            Text("logs: \(logs)")
        }
        .padding()
	}
}
