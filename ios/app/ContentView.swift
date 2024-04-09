import SwiftUI

struct ContentView: View {
	var body: some View {
        NavigationView {
            VStack {
                Button("Start daemon") {
                    let bundlePath = Bundle.main.bundlePath
                    let daemonPath = "\(bundlePath)/kdeconnectd"
                    spawnRoot(daemonPath, [])
                }
            }
            .padding()
        }.navigationTitle("KDE Connect")
	}
}
