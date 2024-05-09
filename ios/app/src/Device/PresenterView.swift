import SwiftUI
import CoreMotion
import UIKit

struct PresenterView: View {
    var device: Binding<ConnectedDevice>

    @State var orientation = UIDevice.current.orientation
    @State var startedGyro = false
    @State var sensitivity: Float = 0.07
    @State var lastDx: Float = 0.0
    @State var lastDy: Float = 0.0

    init(device: Binding<ConnectedDevice>) {
        self.device = device
        motionManager.gyroUpdateInterval = 0.025
    }

    func startGyro() {
        if (!startedGyro) {
            UIImpactFeedbackGenerator(style: .heavy).impactOccurred()
            motionManager.startGyroUpdates(to: .main) { data, err in
                guard let data = data else { return }

                var dx: Float = 0.0
                var dy: Float = 0.0
                switch orientation {
                    case .portrait:
                        dx = -(Float(data.rotationRate.z) * sensitivity)
                        dy = -(Float(data.rotationRate.x) * sensitivity)
                    case .portraitUpsideDown:
                        dx = -(Float(data.rotationRate.z) * sensitivity)
                        dy =  (Float(data.rotationRate.x) * sensitivity)
                    case .landscapeLeft:
                        dx = -(Float(data.rotationRate.z) * sensitivity)
                        dy =  (Float(data.rotationRate.y) * sensitivity)
                    case .landscapeRight:
                        dx = -(Float(data.rotationRate.z) * sensitivity)
                        dy = -(Float(data.rotationRate.y) * sensitivity)
                    default:
                        break
                }

                if dx != 0.0 || dy != 0.0 {
                    sendPresenter(device.id.wrappedValue, NSNumber(value: dx), NSNumber(value: dy))
                }
            }
            startedGyro = true
        }
    }

    func stopGyro() {
        stopPresenter(device.id.wrappedValue)
        motionManager.stopGyroUpdates()
        startedGyro = false
    }

    var body: some View {
        VStack {
            HStack {
                Text("Sensitivity")
                Slider(value: $sensitivity, in: 0...0.1) {
                    Text("Sensitivity")
                } minimumValueLabel: {
                   Image(systemName: "minus")
                } maximumValueLabel: {
                   Image(systemName: "plus")
                } onEditingChanged: { _ in }
                .padding(.leading, 5)
            }.padding(5)
            Spacer()
            Image(systemName: "wand.and.rays")
                .resizable()
                .frame(width: 64, height: 64)
                .padding(64)
                .background(.orange)
                .cornerRadius(32)
                .gesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { _ in
                            startGyro()
                        }
                        .onEnded { _ in
                            stopGyro()
                        }
                )
            Spacer()
        }
        .frame(maxWidth: .infinity)
        .navigationTitle("Presenter")
        .onAppear {
            UIDevice.current.beginGeneratingDeviceOrientationNotifications()
            switch orientation {
                case .portrait, .portraitUpsideDown, .landscapeLeft, .landscapeRight:
                    break;
                default:
                    orientation = .portrait
            }
        }
        .onDisappear {
            stopGyro()
            UIDevice.current.endGeneratingDeviceOrientationNotifications()
        }
        .onReceive(NotificationCenter.default.publisher(for: UIDevice.orientationDidChangeNotification)) { _ in
            let newOrientation = UIDevice.current.orientation
            switch newOrientation {
                case .portrait, .portraitUpsideDown, .landscapeLeft, .landscapeRight:
                    orientation = newOrientation
                default:
                    break
            }
        }
    }
}
