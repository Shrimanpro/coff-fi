// Native iOS layer for Coff-fi: audio session, lock screen / Dynamic Island Now Playing,
// remote commands, AirPlay, the Files folder picker and app lifecycle.
// Playback itself stays in the Rust engine; this file only connects it to iOS.

import AVFoundation
import AVKit
import MediaPlayer
import UIKit
import UniformTypeIdentifiers
import WebKit

@_cdecl("coffi_native_install")
public func coffiNativeInstall() {
    CoffiNative.shared.install()
}

final class CoffiNative: NSObject {
    static let shared = CoffiNative()

    private var nowPlaying: [String: Any] = [:]
    private var resumeAfterInterruption = false
    private var isPlaying = false
    private var commandsAttached = false

    func install() {
        coffi_native_register(CoffiCallbacks(
            now_playing: { json in
                let text = json.map { String(cString: $0) }
                DispatchQueue.main.async { CoffiNative.shared.setTrack(json: text) }
            },
            playback: { hasTrack, playing, position, rate in
                DispatchQueue.main.async { CoffiNative.shared.setPlayback(hasTrack: hasTrack, playing: playing, position: position, rate: rate) }
            },
            request: { what in
                guard let what else { return }
                let w = String(cString: what)
                DispatchQueue.main.async { CoffiNative.shared.handle(request: w) }
            }
        ))
        UIDevice.current.name.withCString { coffi_set_device_name($0) }
        configureSession()
        observeLifecycle()
        FolderAccess.restore()
        SiriSupport.install()
        Haptics.prepare()
        WebViewTuning.applyWhenReady()
        OrientationControl.install()
    }

    // MARK: Audio session

    private func configureSession() {
        let s = AVAudioSession.sharedInstance()
        do {
            // .playback keeps music going with the screen locked or the silent switch on.
            try s.setCategory(.playback, mode: .default, policy: .longFormAudio, options: [])
            // Bigger render buffers mean fewer audio callbacks (the engine buffers far ahead anyway).
            try? s.setPreferredIOBufferDuration(0.046)
            try s.setActive(true)
        } catch {
            NSLog("Coff-fi: audio session error \(error)")
        }
        let nc = NotificationCenter.default
        nc.addObserver(self, selector: #selector(interrupted(_:)), name: AVAudioSession.interruptionNotification, object: s)
        nc.addObserver(self, selector: #selector(routeChanged(_:)), name: AVAudioSession.routeChangeNotification, object: s)
        nc.addObserver(self, selector: #selector(mediaReset(_:)), name: AVAudioSession.mediaServicesWereResetNotification, object: s)
    }

    @objc private func interrupted(_ n: Notification) {
        guard let raw = n.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
        switch type {
        case .began:
            // A call, alarm or Siri took over audio.
            resumeAfterInterruption = isPlaying
            coffi_control(1, 0)
        case .ended:
            let opts = (n.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt).map(AVAudioSession.InterruptionOptions.init) ?? []
            try? AVAudioSession.sharedInstance().setActive(true)
            if resumeAfterInterruption && opts.contains(.shouldResume) {
                coffi_control(0, 0)
            }
            resumeAfterInterruption = false
        @unknown default:
            break
        }
    }

    @objc private func routeChanged(_ n: Notification) {
        guard let raw = n.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: raw) else { return }
        // Headphones unplugged / Bluetooth disconnected: pause instead of blasting the speaker.
        if reason == .oldDeviceUnavailable {
            coffi_control(1, 0)
        }
    }

    @objc private func mediaReset(_ n: Notification) {
        configureSession()
    }

    // MARK: Now Playing (lock screen, Control Center, Dynamic Island)

    private func attachCommands() {
        guard !commandsAttached else { return }
        commandsAttached = true
        let c = MPRemoteCommandCenter.shared()
        c.playCommand.addTarget { _ in coffi_control(0, 0); return .success }
        c.pauseCommand.addTarget { _ in coffi_control(1, 0); return .success }
        c.togglePlayPauseCommand.addTarget { _ in coffi_control(2, 0); return .success }
        c.nextTrackCommand.addTarget { _ in coffi_control(3, 0); return .success }
        c.previousTrackCommand.addTarget { _ in coffi_control(4, 0); return .success }
        c.stopCommand.addTarget { _ in coffi_control(5, 0); return .success }
        c.changePlaybackPositionCommand.addTarget { e in
            guard let e = e as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            coffi_control(6, e.positionTime)
            return .success
        }
        c.likeCommand.localizedTitle = "Favorite"
        c.likeCommand.addTarget { _ in coffi_control(8, 0); return .success }
        c.skipForwardCommand.isEnabled = false
        c.skipBackwardCommand.isEnabled = false
    }

    private func setTrack(json: String?) {
        let center = MPNowPlayingInfoCenter.default()
        guard let json, let data = json.data(using: .utf8),
              let m = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            nowPlaying = [:]
            center.nowPlayingInfo = nil
            return
        }
        attachCommands()
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: m["title"] as? String ?? "",
            MPMediaItemPropertyArtist: m["artist"] as? String ?? "",
            MPMediaItemPropertyAlbumTitle: m["album"] as? String ?? "",
            MPMediaItemPropertyPlaybackDuration: m["duration"] as? Double ?? 0,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: 0.0,
            MPNowPlayingInfoPropertyPlaybackRate: isPlaying ? 1.0 : 0.0,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
        ]
        if let path = m["cover"] as? String, let img = UIImage(contentsOfFile: path) {
            info[MPMediaItemPropertyArtwork] = MPMediaItemArtwork(boundsSize: img.size) { _ in img }
        }
        nowPlaying = info
        center.nowPlayingInfo = info
    }

    private func setPlayback(hasTrack: Bool, playing: Bool, position: Double, rate: Double) {
        isPlaying = playing
        guard hasTrack, !nowPlaying.isEmpty else { return }
        nowPlaying[MPNowPlayingInfoPropertyElapsedPlaybackTime] = position
        nowPlaying[MPNowPlayingInfoPropertyPlaybackRate] = playing ? rate : 0.0
        nowPlaying[MPNowPlayingInfoPropertyDefaultPlaybackRate] = rate
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nowPlaying
    }

    // MARK: Requests from the web UI

    private func handle(request: String) {
        if request.hasPrefix("haptic:") {
            Haptics.play(String(request.dropFirst("haptic:".count)))
            return
        }
        switch request {
        case "orient:any": OrientationControl.allowLandscape(true); return
        case "orient:portrait": OrientationControl.allowLandscape(false); return
        // Nightstand mode: keep the screen on (no auto-lock) while it's showing.
        case "awake:on": UIApplication.shared.isIdleTimerDisabled = true; return
        case "awake:off": UIApplication.shared.isIdleTimerDisabled = false; return
        #if DEBUG
        // Simulator testing only: rotate as if the phone were turned sideways.
        case "orient:test-landscape":
            UIHelpers.keyWindow?.windowScene?.requestGeometryUpdate(.iOS(interfaceOrientations: .landscapeRight)) { e in
                NSLog("Coff-fi: test rotation refused: \(e.localizedDescription)")
            }
            return
        case "awake:test":
            NSLog("Coff-fi: idle timer disabled = \(UIApplication.shared.isIdleTimerDisabled)")
            return
        #endif
        default: break
        }
        switch request {
        case "airplay": AirPlay.showPicker()
        case "pick_folder": FolderAccess.pick(exclude: false)
        case "pick_exclude": FolderAccess.pick(exclude: true)
        case "refresh_shortcuts": SiriSupport.refresh()
        default: NSLog("Coff-fi: unknown native request \(request)")
        }
    }

    // MARK: Lifecycle

    private func observeLifecycle() {
        NotificationCenter.default.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { _ in
            "foreground".withCString { coffi_lifecycle($0) }
        }
        NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { _ in
            "background".withCString { coffi_lifecycle($0) }
        }
    }
}

// MARK: - Helpers

enum UIHelpers {
    static var keyWindow: UIWindow? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows)
            .first { $0.isKeyWindow } ?? UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows)
            .first
    }

    static var topController: UIViewController? {
        var vc = keyWindow?.rootViewController
        while let next = vc?.presentedViewController { vc = next }
        return vc
    }
}

/// The system AirPlay / audio route picker, opened from the Now Playing screen.
enum AirPlay {
    private static var picker: AVRoutePickerView?

    static func showPicker() {
        guard let window = UIHelpers.keyWindow else { return }
        let p = picker ?? AVRoutePickerView(frame: CGRect(x: window.bounds.midX, y: window.bounds.maxY - 80, width: 1, height: 1))
        p.prioritizesVideoDevices = false
        p.alpha = 0.011
        if p.superview == nil { window.addSubview(p) }
        picker = p
        // AVRoutePickerView has no public "present"; tap its internal button.
        if let button = p.subviews.compactMap({ $0 as? UIButton }).first {
            button.sendActions(for: .touchUpInside)
        }
    }
}

/// Music folders chosen in the Files app, kept accessible across launches with
/// security-scoped bookmarks.
final class FolderAccess: NSObject, UIDocumentPickerDelegate {
    private static let key = "coffi.folderBookmarks"
    private static let delegate = FolderAccess()
    private var exclude = false

    static func pick(exclude: Bool) {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder])
        picker.allowsMultipleSelection = false
        delegate.exclude = exclude
        picker.delegate = delegate
        UIHelpers.topController?.present(picker, animated: true)
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        for url in urls {
            guard url.startAccessingSecurityScopedResource() else { continue }
            if let data = try? url.bookmarkData(options: [], includingResourceValuesForKeys: nil, relativeTo: nil) {
                var all = UserDefaults.standard.array(forKey: Self.key) as? [Data] ?? []
                all.append(data)
                UserDefaults.standard.set(all, forKey: Self.key)
            }
            (exclude ? "exclude:" + url.path : url.path).withCString { coffi_folder_picked($0) }
        }
    }

    /// Re-opens access to previously picked folders (needed before the library is rescanned).
    static func restore() {
        let all = UserDefaults.standard.array(forKey: key) as? [Data] ?? []
        var fresh: [Data] = []
        for data in all {
            var stale = false
            guard let url = try? URL(resolvingBookmarkData: data, options: [], relativeTo: nil, bookmarkDataIsStale: &stale) else { continue }
            _ = url.startAccessingSecurityScopedResource()
            fresh.append(stale ? ((try? url.bookmarkData()) ?? data) : data)
        }
        UserDefaults.standard.set(fresh, forKey: key)
    }
}


/// Haptic feedback requested by the web UI ("haptic:<kind>").
enum Haptics {
    private static let light = UIImpactFeedbackGenerator(style: .light)
    private static let medium = UIImpactFeedbackGenerator(style: .medium)
    private static let heavy = UIImpactFeedbackGenerator(style: .heavy)
    private static let soft = UIImpactFeedbackGenerator(style: .soft)
    private static let rigid = UIImpactFeedbackGenerator(style: .rigid)
    private static let selection = UISelectionFeedbackGenerator()
    private static let notification = UINotificationFeedbackGenerator()

    static func prepare() {
        DispatchQueue.main.async {
            [light, medium, soft, rigid].forEach { $0.prepare() }
            selection.prepare()
        }
    }

    static func play(_ kind: String) {
        switch kind {
        case "light": light.impactOccurred(); light.prepare()
        case "medium": medium.impactOccurred(); medium.prepare()
        case "heavy": heavy.impactOccurred(); heavy.prepare()
        case "soft": soft.impactOccurred(); soft.prepare()
        case "rigid": rigid.impactOccurred(); rigid.prepare()
        case "selection": selection.selectionChanged(); selection.prepare()
        case "success": notification.notificationOccurred(.success)
        case "warning": notification.notificationOccurred(.warning)
        case "error": notification.notificationOccurred(.error)
        default: break
        }
    }
}

/// Lets the web UI render at the display's full refresh rate (120 Hz on ProMotion iPhones).
/// WebKit throttles page rendering updates (requestAnimationFrame, JS-driven drags) to ~60 fps
/// by default; that's a WebKit feature flag with no public API, so it's toggled through
/// WKPreferences' feature list when available and silently skipped otherwise.
enum WebViewTuning {
    static func applyWhenReady(attempt: Int = 0) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            guard let web = findWebView(in: UIHelpers.keyWindow) else {
                if attempt < 40 { applyWhenReady(attempt: attempt + 1) }
                return
            }
            disable60FPSCap(web.configuration.preferences)
        }
    }

    private static func findWebView(in view: UIView?) -> WKWebView? {
        guard let view else { return nil }
        if let w = view as? WKWebView { return w }
        for v in view.subviews {
            if let w = findWebView(in: v) { return w }
        }
        return nil
    }

    private static func disable60FPSCap(_ prefs: WKPreferences) {
        let featuresSel = NSSelectorFromString("_features")
        let setSel = NSSelectorFromString("_setEnabled:forFeature:")
        let cls: AnyObject = WKPreferences.self
        guard cls.responds(to: featuresSel), prefs.responds(to: setSel),
              let list = cls.perform(featuresSel)?.takeUnretainedValue() as? [NSObject] else {
            NSLog("Coff-fi: WebKit feature list unavailable; page rendering stays at the default rate")
            return
        }
        typealias SetEnabled = @convention(c) (AnyObject, Selector, Bool, AnyObject) -> Void
        let fn = unsafeBitCast(prefs.method(for: setSel), to: SetEnabled.self)
        var found = false
        for f in list where (f.value(forKey: "key") as? String) == "PreferPageRenderingUpdatesNear60FPSEnabled" {
            fn(prefs, setSel, false, f)
            found = true
        }
        NSLog("Coff-fi: 60 fps page-rendering cap \(found ? "disabled" : "not found")")
    }
}


/// Portrait everywhere, landscape only while Now Playing is open with Nightstand mode on.
/// Tauri's root view controller (TaoUIViewController) exposes a settable
/// `supportedInterfaceOrientations`, which is what iOS consults, so the mask is set there.
enum OrientationControl {
    private static var mask: UIInterfaceOrientationMask = .portrait

    /// Lock to portrait as soon as Tauri's window exists.
    static func install(attempt: Int = 0) {
        DispatchQueue.main.asyncAfter(deadline: .now() + (attempt == 0 ? 0 : 0.1)) {
            if !apply(), attempt < 100 { install(attempt: attempt + 1) }
        }
    }

    static func allowLandscape(_ on: Bool) {
        mask = on ? .allButUpsideDown : .portrait
        apply()
        if !on, let scene = UIHelpers.keyWindow?.windowScene, scene.interfaceOrientation != .portrait {
            // Leaving Now Playing while sideways: rotate the app back upright.
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: .portrait)) { _ in }
        }
    }

    /// Returns false while there's no root view controller yet.
    @discardableResult
    private static func apply() -> Bool {
        let sel = NSSelectorFromString("setSupportedInterfaceOrientations:")
        var found = false
        for scene in UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }) {
            for w in scene.windows {
                guard let vc = w.rootViewController else { continue }
                found = true
                if vc.responds(to: sel) {
                    typealias Setter = @convention(c) (AnyObject, Selector, UInt) -> Void
                    let set = unsafeBitCast(vc.method(for: sel), to: Setter.self)
                    set(vc, sel, mask.rawValue)
                }
                vc.setNeedsUpdateOfSupportedInterfaceOrientations()
            }
        }
        if found, mask == .portrait, let scene = UIHelpers.keyWindow?.windowScene, scene.interfaceOrientation != .portrait {
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: .portrait)) { _ in }
        }
        return found
    }
}
