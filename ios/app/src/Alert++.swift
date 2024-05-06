//
//  Alert++.swift
//  PsychicPaper
//
//  Created by Hariz Shirazi on 2023-02-04.
//

fileprivate let errorString = NSLocalizedString("Error", comment: "Title to display when displaying an error")
fileprivate let okString = NSLocalizedString("OK", comment: "Default string to confirm an action")
fileprivate let cancelString = NSLocalizedString("Cancel", comment: "Default string to cancel an action")

#if canImport(UIKit)
import UIKit

// Thanks suslocation!
var currentUIAlertController: UIAlertController?

extension UIApplication {
    func dismissAlert(animated: Bool) {
        DispatchQueue.main.async {
            currentUIAlertController?.dismiss(animated: animated)
        }
    }

    func alert(title: String = errorString, body: String, animated: Bool = true, withButton: Bool = true) {
        // ==== do not uncomment ====
//        DispatchQueue.main.async {
        var body = body
        if title == errorString {
            if let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String,
               let name = Bundle.main.infoDictionary?["CFBundleName"] as? String {
                body += "\n\n\(name) v\(version), iOS \(UIDevice.current.systemVersion)"
            }
        }
        currentUIAlertController = UIAlertController(title: title, message: body, preferredStyle: .alert)
        if withButton { currentUIAlertController?.addAction(.init(title: okString, style: .cancel)) }
        currentUIAlertController?.view.tintColor = UIColor(named: "AccentColor")
        self.present(alert: currentUIAlertController!)
//        }
    }

    func progressAlert(title: String, body: String = "", animated: Bool = true, noCancel: Bool = true) {
        DispatchQueue.main.async {
            currentUIAlertController = UIAlertController(title: title, message: body, preferredStyle: .alert)
            if body != "" {
                currentUIAlertController?.textFields?.forEach({$0.textAlignment = .left})
            }

            let indicator = UIActivityIndicatorView(frame: CGRectMake(5,5,50,50))
            indicator.hidesWhenStopped = true
            indicator.style = .medium
            indicator.startAnimating()

            currentUIAlertController?.view.addSubview(indicator)

            if !noCancel { currentUIAlertController?.addAction(.init(title: "Cancel", style: .cancel)) }
            currentUIAlertController?.view.tintColor = UIColor(named: "AccentColor")
            self.present(alert: currentUIAlertController!)
        }
    }

    func confirmAlert(title: String = errorString, body: String, confirmTitle: String = okString, cancelTitle: String = cancelString, onOK: @escaping () -> (), noCancel: Bool) {
        DispatchQueue.main.async {
            var body = body
            if title == errorString {
                if let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String,
                   let name = Bundle.main.infoDictionary?["CFBundleName"] as? String {
                    body += "\n\n\(name) v\(version), iOS \(UIDevice.current.systemVersion)"
                }
            }
            currentUIAlertController = UIAlertController(title: title, message: body, preferredStyle: .alert)
            if !noCancel {
                currentUIAlertController?.addAction(.init(title: cancelTitle, style: .cancel))
            }
            currentUIAlertController?.addAction(.init(title: confirmTitle, style: noCancel ? .cancel : .default, handler: { _ in
                onOK()
            }))
            currentUIAlertController?.view.tintColor = UIColor(named: "AccentColor")
            self.present(alert: currentUIAlertController!)
        }
    }

    func choiceAlert(title: String = "Error", body: String, confirmTitle: String = okString, cancelTitle: String = cancelString, yesAction: @escaping () -> (), noAction: @escaping () -> ()) {
        DispatchQueue.main.async {
            var body = body
            if title == errorString {
                if let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String,
                   let name = Bundle.main.infoDictionary?["CFBundleName"] as? String {
                    body += "\n\n\(name) v\(version), iOS \(UIDevice.current.systemVersion)"
                }
            }
            currentUIAlertController = UIAlertController(title: title, message: body, preferredStyle: .alert)
            currentUIAlertController?.addAction(.init(title: cancelTitle, style: .cancel, handler: { _ in
                noAction()
            }))
            currentUIAlertController?.addAction(.init(title: confirmTitle, style: .default, handler: { _ in
                yesAction()
            }))
            currentUIAlertController?.view.tintColor = UIColor(named: "AccentColor")
            self.present(alert: currentUIAlertController!)
        }
    }

    func confirmAlertDestructive(title: String = "Error", body: String, onOK: @escaping () -> (), onCancel: @escaping () -> () = {}, destructActionText: String) {
        DispatchQueue.main.async {
            var body = body
            if title == errorString {
                if let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String,
                   let name = Bundle.main.infoDictionary?["CFBundleName"] as? String {
                    body += "\n\n\(name) v\(version), iOS \(UIDevice.current.systemVersion)"
                }
            }
            currentUIAlertController = UIAlertController(title: title, message: body, preferredStyle: .alert)
            currentUIAlertController?.addAction(.init(title: destructActionText, style: .destructive, handler: { _ in
                onOK()
            }))
            currentUIAlertController?.addAction(.init(title: "Cancel", style: .cancel, handler: { _ in
                onCancel()
            }))
            currentUIAlertController?.view.tintColor = UIColor(named: "AccentColor")
            self.present(alert: currentUIAlertController!)
        }
    }

    func change(title: String = errorString, body: String, removeSubViews: Bool = false, addCancelWithTitle: String? = nil, onCancel: @escaping () -> () = {}) {
        var body = body
        if title == errorString {
            if let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String,
               let name = Bundle.main.infoDictionary?["CFBundleName"] as? String {
                body += "\n\n\(name) v\(version), iOS \(UIDevice.current.systemVersion)"
            }
        }
        DispatchQueue.main.async {
            if removeSubViews {
                currentUIAlertController?.view.subviews[safe:1]?.removeFromSuperview() // removes any spinners
            }
            currentUIAlertController?.title = title
            currentUIAlertController?.message = body
            if let addCancelWithTitle {
                currentUIAlertController?.addAction(.init(title: addCancelWithTitle, style: .cancel, handler: { _ in
                    onCancel()
                }))
            }
        }
    }

    func changeBody(_ body: String, noDebugInfo: Bool = false, removeSubViews: Bool = false) {
        var body = body
        if currentUIAlertController?.title == errorString && !noDebugInfo {
            if let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String,
               let name = Bundle.main.infoDictionary?["CFBundleName"] as? String {
                body += "\n\n\(name) v\(version), iOS \(UIDevice.current.systemVersion)"
            }
        }
        DispatchQueue.main.async {
            if removeSubViews {
                currentUIAlertController?.view.subviews[safe:1]?.removeFromSuperview() // removes any spinners
            }
            currentUIAlertController?.message = body
        }
    }

    func changeTitle(_ title: String, removeSubViews: Bool = false) {
        if title == errorString {
            if let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String,
               let name = Bundle.main.infoDictionary?["CFBundleName"] as? String {
                currentUIAlertController?.message? += "\n\n\(name) v\(version), iOS \(UIDevice.current.systemVersion)"
            }
        }
        DispatchQueue.main.async {
            if removeSubViews {
                currentUIAlertController?.view.subviews[safe:1]?.removeFromSuperview() // removes any spinners
            }
            currentUIAlertController?.title = title
        }
    }

    func present(alert: UIAlertController) {
        if var topController = (UIApplication.shared.connectedScenes.first as? UIWindowScene)?.windows[0].rootViewController {
            while let presentedViewController = topController.presentedViewController {
                topController = presentedViewController
            }

            topController.present(alert, animated: true)
            // topController should now be your topmost view controller
        }
    }
}
#else
import AppKit

typealias UIApplication = NSApplication // for pre-existing code
extension NSApplication {
    func alert(title: String = errorString, body: String, animated: Bool = true, withButton: Bool = true) {
        DispatchQueue.main.async {
            let currentNSAlert = NSAlert()
            currentNSAlert.messageText = title
            currentNSAlert.informativeText = body
            if withButton {
                currentNSAlert.addButton(withTitle: okString)
            }

            currentNSAlert.runModal()
        }
    }
}
#endif
