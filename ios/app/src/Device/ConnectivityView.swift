import SwiftUI

// https://www.objc.io/blog/2020/02/18/a-signal-strength-indicator/
struct Divided<S: Shape>: Shape {
    var amount: CGFloat
    var shape: S
    func path(in rect: CGRect) -> Path {
        shape.path(in: rect.divided(atDistance: amount * rect.height, from: .maxYEdge).slice)
    }
}

extension Shape {
    func divided(amount: CGFloat) -> Divided<Self> {
        return Divided(amount: amount, shape: self)
    }
}

struct SignalStrengthIndicator: View {
    var bars: Int
    var totalBars: Int = 5
    var body: some View {
        HStack {
            ForEach(0..<totalBars) { bar in
                RoundedRectangle(cornerRadius: 4)
                    .divided(amount: (CGFloat(bar) + 1) / CGFloat(self.totalBars))
                    .fill(Color.primary.opacity(bar < self.bars ? 1 : 0.3))
            }
        }
    }
}

struct ConnectivityView: View {
    var device: Binding<ConnectedDevice>
    var refresh: () -> Void
    var body: some View {
        VStack {
            List {
                if (device.connectivity.count > 0) {
                    ForEach(device.connectivity) { signal in
                        HStack {
                            Text(signal.id.wrappedValue).font(.system(.body, design: .monospaced)).padding(.trailing, 8)
                            SignalStrengthIndicator(bars: signal.strength.wrappedValue).frame(maxWidth: 50, maxHeight: 25)
                            Text(signal.type.wrappedValue)
                            Spacer()
                        }
                    }
                } else {
                    Text("This device has nothing in its connectivity report.").padding(.vertical, 4)
                }
            }
        }
        .navigationTitle("Connectivity Report")
        .refreshable {
            refresh()
        }
    }
}
