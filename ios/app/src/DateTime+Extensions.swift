extension Date {
    static func - (lhs: Date, rhs: Date) -> TimeInterval {
        return lhs.timeIntervalSinceReferenceDate - rhs.timeIntervalSinceReferenceDate
    }
}

struct MediaPlayerFormatStyle: FormatStyle {
    func format(_ value: Float) -> String {
        let formatter = DateComponentsFormatter()
        if value > 3600 {
            formatter.allowedUnits = [.hour, .minute, .second]
        } else {
            formatter.allowedUnits = [.minute, .second]
        }
        formatter.unitsStyle = .positional
        formatter.zeroFormattingBehavior = .pad
        return formatter.string(from: TimeInterval(value)) ?? ""
    }
}

extension FormatStyle where Self == MediaPlayerFormatStyle {
    static var mediaLength: MediaPlayerFormatStyle { MediaPlayerFormatStyle() }
}
