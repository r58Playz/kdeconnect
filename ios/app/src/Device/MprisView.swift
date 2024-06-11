import SwiftUI

enum MprisAction: Int {
	case Seek
	case Volume
	case LoopStatusNone
	case LoopStatusTrack
    case LoopStatusPlaylist
    case Position
    case Shuffle
    case Play
    case Pause
    case PlayPause
    case Stop
    case Next
    case Previous
}

struct MprisView: View {
    @Binding var player: ConnectedDeviceMprisPlayer 
    @State var cachedPosition: Int = -1
	var id: String

    var body: some View {
        let positionProxy = Binding<Float>(
            get: {cachedPosition != -1 ? Float(cachedPosition) / 1000.0 : Float(player.position) / 1000.0},
            set: {cachedPosition = Int($0 * 1000)}
        )

        let lengthProxy = Binding<Float>(
            get: {
                var length = Float(player.length) / 1000.0
                if length < 0 { length = 0.0 }
                return length
            },
            set: { _ in }
        )
        Section(player.id) {
            VStack {
                HStack {
                    VStack {
                        if player.albumArt != "", let image = UIImage(contentsOfFile: player.albumArt) {
                            Image(uiImage: image)
                            .resizable().aspectRatio(contentMode: .fit).frame(height: 256.0)
                        } else {
                            Image(systemName: "music.note")
                            .resizable().aspectRatio(contentMode: .fit).frame(height: 96.0).padding(.all, 80)
                        }
                    }
                    .frame(minWidth: 256.0, minHeight: 256.0)
                    .background(.uikitSecondarySystemBackground)
                    .cornerRadius(8)
                }
                Text(player.title).font(.title3).bold().lineLimit(1).truncationMode(.tail)
                Text(player.artist).lineLimit(1).truncationMode(.tail)
                Text(player.album).foregroundStyle(.uikitSecondaryLabel).lineLimit(1).truncationMode(.tail)

                VStack {
                    MusicSlider(
                        value: positionProxy,
                        inRange: 0...lengthProxy.wrappedValue,
                        activeFillColor: .uikitSecondaryLabel,
                        fillColor: .uikitTertiaryLabel,
                        emptyColor: .uikitSecondarySystemBackground,
                        height: 6
                    ) { editing in 
						if !editing {
							requestPlayerAction(id, player.id, MprisAction.Position.rawValue as NSNumber, cachedPosition as NSNumber)
							// wait a bit as by then we probably have recieved a response that the position has changed and it'll be seamless
							DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
								cachedPosition = -1
							}
						}
                    }
                    HStack {
                        Text(positionProxy.wrappedValue, format: .mediaLength).font(.caption)
                        Spacer()
                        Text(lengthProxy.wrappedValue, format: .mediaLength).font(.caption)
                    }
                }

                HStack {
                    Spacer()
                    Button {
                        requestPlayerAction(id, player.id, MprisAction.Previous.rawValue as NSNumber, 0 as NSNumber)
                    } label: {
                        Image(systemName: "backward.fill")
                    }.foregroundColor(.uikitLabel).font(.title)
                    Button {
                        requestPlayerAction(id, player.id, MprisAction.PlayPause.rawValue as NSNumber, 0 as NSNumber)
                    } label: {
                        if player.isPlaying {
                            Image(systemName: "pause.fill")
                        } else {
                            Image(systemName: "play.fill")
                        }
                    }.foregroundColor(.uikitLabel).font(.system(size: 48))
                    Button {
                        requestPlayerAction(id, player.id, MprisAction.Next.rawValue as NSNumber, 0 as NSNumber)
                    } label: {
                        Image(systemName: "forward.fill")
                    }.foregroundColor(.uikitLabel).font(.title)
                    Spacer()
                }
            }
        }.buttonStyle(PlainButtonStyle())
    }
}

struct MprisPlayersView: View {
    @Binding var device: ConnectedDevice
    var refresh: () -> Void

    var body: some View {
        var sortedPlayers: [Binding<ConnectedDeviceMprisPlayer>] {
            $device.player.sorted { $x, $y in
                if (x.length > 0) != (y.length > 0) {
                    return x.length > 0
                } else {
                    return x.id > y.id
                }
            }
        }
        List {
            ForEach(sortedPlayers) { $player in
                // hide player controls for players that don't have much info
                if (player.length > 0) {
                    MprisView(player: $player, id: device.id)
                } else {
                    Section(player.id) {
                        Text("This player does not have length information, so it has been hidden.").padding(.vertical, 4)
                    }
                }
            }
        }
        .navigationTitle("Media")
        .refreshable {
            refresh()
            requestPlayers(device.id)
            for player in device.player {
                requestPlayer(device.id, player.id)
            }
        }
    }
}
