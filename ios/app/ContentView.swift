import SwiftUI

struct ContentView: View {
    var pipe = Pipe()
    var pipe2 = Pipe()
    @State var logs: String = "";
    //public func openConsolePipe() {
    //        setvbuf(stdout, nil, _IONBF, 0)
    //        dup2(pipe.fileHandleForWriting.fileDescriptor,
    //             STDOUT_FILENO)
    //             dup2(pipe2.fileHandleForWriting.fileDescriptor,
    //                              STDERR_FILENO)
    //        // listening on the readabilityHandler
    //        pipe.fileHandleForReading.readabilityHandler = { handle in
    //            let data = handle.availableData
    //            let str = String(data: data, encoding: .ascii) ?? "[*] <Non-ascii data of size\(data.count)>\n"
    //            DispatchQueue.main.async {
    //                log += str + "\n"
    //            }
    //        }
    //        pipe2.fileHandleForReading.readabilityHandler = { handle in
    //            let data = handle.availableData
    //            let str = String(data: data, encoding: .ascii) ?? "[ERR] <Non-ascii data of size\(data.count)>\n"
    //            DispatchQueue.main.async {
    //                log += "[ERR] " + str + "\n"
    //            }
    //        }
    //    }
	var body: some View {
        VStack {
            Text("KDE Connect")
                .font(.title)
            Button("Start daemon") {
                logs += "Starting daemon...\n"
                let bundlePath = Bundle.main.bundlePath
                let daemonPath = "\(bundlePath)/kdeconnectd"
                spawnRoot(daemonPath, [])
            }
            //.onAppear {
            //    openConsolePipe()
            //}
            Spacer()
            Text("logs: \(logs)")
        }
        .padding()
	}
}
