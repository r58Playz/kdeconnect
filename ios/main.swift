import Foundation
import UIKit
import libroot

var trollstore = false

if CommandLine.argc > 2 {
    print("usage: \(CommandLine.arguments[0]) [--trollstore]")
    exit(1)
}

if (CommandLine.argc == 2 && CommandLine.arguments[1] == "--trollstore") {
    trollstore = true
}

var devicetype = K_CONNECT_FFI_DEVICE_TYPE_PHONE

func directoryExistsAtPath(_ path: String) -> Bool {
    var isDirectory : ObjCBool = true
    let exists = FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory)
    return exists && isDirectory.boolValue
}

if !directoryExistsAtPath("/var/mobile/kdeconnect") {
    try FileManager.default.createDirectory(atPath: "/var/mobile/kdeconnect", withIntermediateDirectories: false)
}

if !FileManager.default.fileExists(atPath: "/var/mobile/kdeconnect/type") {
    try "phone".write(toFile: "/var/mobile/kdeconnect/type", atomically: true, encoding: .utf8)
}

if !FileManager.default.fileExists(atPath: "/var/mobile/kdeconnect/name") {
    try UIDevice.current.name.write(toFile: "/var/mobile/kdeconnect/name", atomically: true, encoding: .utf8)
}

var devicetypestr = try String(contentsOfFile: "/var/mobile/kdeconnect/type")

switch devicetypestr {
    case "phone":
        devicetype = K_CONNECT_FFI_DEVICE_TYPE_PHONE
        break
    case "tablet":
        devicetype = K_CONNECT_FFI_DEVICE_TYPE_TABLET
        break
    case "tv":
        devicetype = K_CONNECT_FFI_DEVICE_TYPE_TV
        break
    case "desktop":
        devicetype = K_CONNECT_FFI_DEVICE_TYPE_DESKTOP
        break
    case "laptop":
        devicetype = K_CONNECT_FFI_DEVICE_TYPE_LAPTOP
        break
    default:
        print("invalid device type: \(devicetypestr)")
        exit(1)
}

var name = try String(contentsOfFile: "/var/mobile/kdeconnect/name")

// FIXME: We need to move more stuff over to Swift!
objc_main(name, KConnectFfiDeviceType_t(devicetype.rawValue), trollstore)
