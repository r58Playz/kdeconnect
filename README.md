# kdeconnectjb
KDE Connect implementation in Rust and application for jailbroken/TrollStore iOS

## Building the iOS implementation
1. Install [Theos](https://theos.dev)
2. Set up building Rust for iOS; make sure `cargo build --target aarch64-apple-ios` works for a clean Rust project
3. Run `./make.sh do` in `kdeconnectjb` directory to build for iOS and deploy to your configured Theos device

## Protocol support
 - [x] `kdeconnect.identity`
 - [x] `kdeconnect.pair`
 - [x] `kdeconnect.battery`
 - [x] `kdeconnect.battery.request`
 - [x] `kdeconnect.clipboard`
 - [x] `kdeconnect.clipboard.connect`
 - [x] `kdeconnect.connectivity_report`
 - [x] `kdeconnect.connectivity_report.request`
 - [ ] `kdeconnect.contacts.request_all_uids_timestamps`
 - [ ] `kdeconnect.contacts.request_vcards_by_uid`
 - [ ] `kdeconnect.contacts.response_uids_timestamps`
 - [ ] `kdeconnect.contacts.response_vcards`
 - [x] `kdeconnect.findmyphone.request`
 - [ ] `kdeconnect.lock`
 - [ ] `kdeconnect.lock.request`
 - [x] `kdeconnect.mousepad.echo`
 - [x] `kdeconnect.mousepad.keyboardstate` (ios client does not support outgoing)
 - [x] `kdeconnect.mousepad.request` (ios client does not support incoming)
 - [x] `kdeconnect.mpris` (ios client buggy)
 - [x] `kdeconnect.mpris.request` (ios client buggy)
 - [ ] `kdeconnect.notification`
 - [ ] `kdeconnect.notification.action`
 - [ ] `kdeconnect.notification.reply`
 - [ ] `kdeconnect.notification.request`
 - [x] `kdeconnect.ping`
 - [x] `kdeconnect.presenter` (ios client does not support incoming)
 - [ ] `kdeconnect.runcommand`
 - [ ] `kdeconnect.runcommand.request`
 - [ ] `kdeconnect.sftp`
 - [ ] `kdeconnect.sftp.request`
 - [x] `kdeconnect.share.request`
 - [x] `kdeconnect.share.request.update`
 - [ ] `kdeconnect.sms.attachment_file`
 - [ ] `kdeconnect.sms.messages`
 - [ ] `kdeconnect.sms.request`
 - [ ] `kdeconnect.sms.request_attachment`
 - [ ] `kdeconnect.sms.request_conversation`
 - [ ] `kdeconnect.sms.request_conversations`
 - [x] `kdeconnect.systemvolume`
 - [x] `kdeconnect.systemvolume.request`
 - [ ] `kdeconnect.telephony`
 - [ ] `kdeconnect.telephony.request_mute`
