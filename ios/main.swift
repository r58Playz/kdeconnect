import Foundation

var trollstore = false

if CommandLine.argc < 3 || CommandLine.argc > 4 {
    print("usage: \(CommandLine.arguments[0]) <device_name> <device_type> [--trollstore]")
    exit(1)
}

if (CommandLine.argc == 4 && CommandLine.arguments[3] == "--trollstore") {
    trollstore = true
}

var devicetype = K_CONNECT_FFI_DEVICE_TYPE_PHONE

switch CommandLine.arguments[2] {
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
        print("invalid device type: \(CommandLine.arguments[2])")
        exit(1)
}

// FIXME: We need to move more stuff over to Swift!
objc_main(CommandLine.arguments[1], KConnectFfiDeviceType_t(devicetype.rawValue), trollstore)
