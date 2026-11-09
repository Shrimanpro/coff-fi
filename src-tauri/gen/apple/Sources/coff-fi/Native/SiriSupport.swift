// Siri & Shortcuts for Coff-fi: "Hey Siri, play my Workout playlist in Coff-fi", albums,
// artists and songs. Entities are looked up in the Rust library (coffi_find) and played with
// coffi_play, so Siri sees exactly what the app shows, local files or Navidrome.

import AppIntents
import CoreSpotlight
import Foundation
import Intents
import ObjectiveC
import UIKit

// MARK: - Bridge helpers

struct LibraryItem: Decodable {
    let kind: String
    let id: String
    let name: String
    let subtitle: String
}

enum Library {
    /// Waits for the Rust core when Siri launched the app in the background.
    static func waitUntilReady() async {
        for _ in 0..<100 where !coffi_ready() {
            try? await Task.sleep(nanoseconds: 100_000_000)
        }
    }

    static func find(_ kind: String, _ query: String) async -> [LibraryItem] {
        await waitUntilReady()
        guard let ptr = kind.withCString({ k in query.withCString { q in coffi_find(k, q) } }) else { return [] }
        defer { coffi_string_free(ptr) }
        let data = Data(String(cString: ptr).utf8)
        return (try? JSONDecoder().decode([LibraryItem].self, from: data)) ?? []
    }

    @discardableResult
    static func play(_ kind: String, _ id: String, shuffle: Bool) async -> Bool {
        await waitUntilReady()
        return kind.withCString { k in id.withCString { i in coffi_play(k, i, shuffle) } }
    }
}

// MARK: - Entities

protocol LibraryEntity: AppEntity, IndexedEntity where ID == String {
    static var kind: String { get }
    var id: String { get }
    var name: String { get }
    var subtitle: String { get }
    init(_ item: LibraryItem)
}

extension LibraryEntity {
    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", subtitle: subtitle.isEmpty ? nil : "\(subtitle)")
    }
}

protocol LibraryEntityQuery: EntityStringQuery where Entity: LibraryEntity {}

extension LibraryEntityQuery {
    func entities(for identifiers: [String]) async throws -> [Entity] {
        // Ids are stable library ids; resolve their names from the suggestions when possible.
        let known = await Library.find(Entity.kind, "")
        return identifiers.map { id in
            known.first { $0.id == id }.map(Entity.init) ?? Entity(LibraryItem(kind: Entity.kind, id: id, name: id, subtitle: ""))
        }
    }

    func entities(matching string: String) async throws -> [Entity] {
        await Library.find(Entity.kind, string).map(Entity.init)
    }

    func suggestedEntities() async throws -> [Entity] {
        await Library.find(Entity.kind, "").map(Entity.init)
    }
}

struct PlaylistQuery: LibraryEntityQuery { typealias Entity = PlaylistEntity }
struct AlbumQuery: LibraryEntityQuery { typealias Entity = AlbumEntity }
struct ArtistQuery: LibraryEntityQuery { typealias Entity = ArtistEntity }
struct SongQuery: LibraryEntityQuery { typealias Entity = SongEntity }

struct PlaylistEntity: LibraryEntity {
    static let kind = "playlist"
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Playlist"
    static let defaultQuery = PlaylistQuery()
    let id: String
    @Property(title: "Name") var name: String
    var subtitle: String
    init(_ i: LibraryItem) { id = i.id; subtitle = i.subtitle; name = i.name }
}

struct AlbumEntity: LibraryEntity {
    static let kind = "album"
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Album"
    static let defaultQuery = AlbumQuery()
    let id: String
    @Property(title: "Name") var name: String
    var subtitle: String
    init(_ i: LibraryItem) { id = i.id; subtitle = i.subtitle; name = i.name }
}

struct ArtistEntity: LibraryEntity {
    static let kind = "artist"
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Artist"
    static let defaultQuery = ArtistQuery()
    let id: String
    @Property(title: "Name") var name: String
    var subtitle: String
    init(_ i: LibraryItem) { id = i.id; subtitle = i.subtitle; name = i.name }
}

struct SongEntity: LibraryEntity {
    static let kind = "song"
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Song"
    static let defaultQuery = SongQuery()
    let id: String
    @Property(title: "Name") var name: String
    var subtitle: String
    init(_ i: LibraryItem) { id = i.id; subtitle = i.subtitle; name = i.name }
}

// MARK: - Intents (AudioPlaybackIntent: plays without opening the app)

struct PlayPlaylistIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play Playlist"
    static let description = IntentDescription("Plays a playlist in Coff-fi.")
    @Parameter(title: "Playlist") var playlist: PlaylistEntity
    @Parameter(title: "Shuffle", default: false) var shuffle: Bool
    static var parameterSummary: some ParameterSummary { Summary("Play \(\.$playlist)") { \.$shuffle } }

    func perform() async throws -> some IntentResult & ProvidesDialog {
        guard await Library.play(PlaylistEntity.kind, playlist.id, shuffle: shuffle) else {
            return .result(dialog: "I couldn't find \(playlist.name) in Coff-fi.")
        }
        return .result(dialog: "Playing \(playlist.name).")
    }
}

struct PlayAlbumIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play Album"
    static let description = IntentDescription("Plays an album in Coff-fi.")
    @Parameter(title: "Album") var album: AlbumEntity
    @Parameter(title: "Shuffle", default: false) var shuffle: Bool
    static var parameterSummary: some ParameterSummary { Summary("Play \(\.$album)") { \.$shuffle } }

    func perform() async throws -> some IntentResult & ProvidesDialog {
        guard await Library.play(AlbumEntity.kind, album.id, shuffle: shuffle) else {
            return .result(dialog: "I couldn't find \(album.name) in Coff-fi.")
        }
        return .result(dialog: "Playing \(album.name).")
    }
}

struct PlayArtistIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play Artist"
    static let description = IntentDescription("Plays songs by an artist in Coff-fi.")
    @Parameter(title: "Artist") var artist: ArtistEntity
    @Parameter(title: "Shuffle", default: true) var shuffle: Bool
    static var parameterSummary: some ParameterSummary { Summary("Play \(\.$artist)") { \.$shuffle } }

    func perform() async throws -> some IntentResult & ProvidesDialog {
        guard await Library.play(ArtistEntity.kind, artist.id, shuffle: shuffle) else {
            return .result(dialog: "I couldn't find \(artist.name) in Coff-fi.")
        }
        return .result(dialog: "Playing \(artist.name).")
    }
}

struct PlaySongIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play Song"
    static let description = IntentDescription("Plays a song in Coff-fi.")
    @Parameter(title: "Song") var song: SongEntity
    static var parameterSummary: some ParameterSummary { Summary("Play \(\.$song)") }

    func perform() async throws -> some IntentResult & ProvidesDialog {
        guard await Library.play(SongEntity.kind, song.id, shuffle: false) else {
            return .result(dialog: "I couldn't find \(song.name) in Coff-fi.")
        }
        return .result(dialog: "Playing \(song.name).")
    }
}

// MARK: - Siri phrases (App Shortcuts need the app name in the phrase)

struct CoffiShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: PlayPlaylistIntent(),
            phrases: [
                "Play my \(\.$playlist) playlist in \(.applicationName)",
                "Play \(\.$playlist) in \(.applicationName)",
                "Play my \(\.$playlist) playlist on \(.applicationName)",
                "Play the \(\.$playlist) playlist in \(.applicationName)",
            ],
            shortTitle: "Play Playlist",
            systemImageName: "music.note.list"
        )
        AppShortcut(
            intent: PlayAlbumIntent(),
            phrases: [
                "Play the album \(\.$album) in \(.applicationName)",
                "Play \(\.$album) album in \(.applicationName)",
            ],
            shortTitle: "Play Album",
            systemImageName: "square.stack"
        )
        AppShortcut(
            intent: PlayArtistIntent(),
            phrases: [
                "Play songs by \(\.$artist) in \(.applicationName)",
                "Play \(\.$artist) in \(.applicationName)",
            ],
            shortTitle: "Play Artist",
            systemImageName: "music.mic"
        )
        AppShortcut(
            intent: PlaySongIntent(),
            phrases: [
                "Play the song \(\.$song) in \(.applicationName)",
                "Play \(\.$song) song in \(.applicationName)",
            ],
            shortTitle: "Play Song",
            systemImageName: "music.note"
        )
    }
}

// MARK: - Registration, Spotlight index, INPlayMediaIntent

enum SiriSupport {
    static func install() {
        // App Intents / App Shortcuts work without any entitlement. The classic SiriKit media
        // intent needs the Siri capability (paid developer account), so it's switched by the
        // CoffiSiriKit Info.plist key; requesting authorization without the entitlement crashes.
        #if !targetEnvironment(simulator)
        if Bundle.main.object(forInfoDictionaryKey: "CoffiSiriKit") as? Bool == true {
            PlayMediaHandler.installInAppHandling()
            INPreferences.requestSiriAuthorization { _ in }
        }
        #endif
    }

    /// Called after every library sync: refreshes Siri's phrase parameters and the Spotlight
    /// index that lets Apple Intelligence Siri resolve "my workout playlist" without exact names.
    static func refresh() {
        CoffiShortcuts.updateAppShortcutParameters()
        Task.detached {
            let playlists = await Library.find("playlist", "").map(PlaylistEntity.init)
            let albums = await Library.find("album", "").map(AlbumEntity.init)
            let artists = await Library.find("artist", "").map(ArtistEntity.init)
            let index = CSSearchableIndex.default()
            try? await index.deleteAppEntities(ofType: PlaylistEntity.self)
            try? await index.deleteAppEntities(ofType: AlbumEntity.self)
            try? await index.deleteAppEntities(ofType: ArtistEntity.self)
            try? await index.indexAppEntities(playlists)
            try? await index.indexAppEntities(albums)
            try? await index.indexAppEntities(artists)
        }
    }
}

/// Handles SiriKit's media intent ("Hey Siri, play my workout playlist") in-app, once Siri knows
/// Coff-fi as a music app. Tauri owns the app delegate, so the handler hook is added at runtime.
final class PlayMediaHandler: NSObject, INPlayMediaIntentHandling {
    static let shared = PlayMediaHandler()

    static func installInAppHandling() {
        DispatchQueue.main.async {
            guard let delegate = UIApplication.shared.delegate else { return }
            let cls: AnyClass = type(of: delegate)
            let sel = NSSelectorFromString("application:handlerForIntent:")
            guard !delegate.responds(to: sel) else { return }
            let block: @convention(block) (AnyObject, UIApplication, INIntent) -> Any? = { _, _, intent in
                intent is INPlayMediaIntent ? PlayMediaHandler.shared : nil
            }
            class_addMethod(cls, sel, imp_implementationWithBlock(block), "@@:@@")
        }
    }

    private func candidates(_ search: INMediaSearch?) async -> [LibraryItem] {
        let name = search?.mediaName ?? search?.albumName ?? search?.artistName ?? ""
        var kinds: [String]
        switch search?.mediaType {
        case .playlist: kinds = ["playlist"]
        case .album: kinds = ["album"]
        case .artist: kinds = ["artist"]
        case .song: kinds = ["song"]
        default: kinds = ["playlist", "album", "artist", "song"]
        }
        if search?.albumName != nil && search?.mediaName == nil { kinds = ["album"] }
        if search?.artistName != nil && search?.mediaName == nil && search?.albumName == nil { kinds = ["artist"] }
        for k in kinds {
            let found = await Library.find(k, name)
            if !found.isEmpty { return found }
        }
        return []
    }

    func resolveMediaItems(for intent: INPlayMediaIntent) async -> [INPlayMediaMediaItemResolutionResult] {
        let found = await candidates(intent.mediaSearch)
        guard let first = found.first else {
            return [INPlayMediaMediaItemResolutionResult(mediaItemResolutionResult: .unsupported())]
        }
        let type: INMediaItemType = ["playlist": .playlist, "album": .album, "artist": .artist][first.kind] ?? .song
        let item = INMediaItem(identifier: "\(first.kind)|\(first.id)", title: first.name, type: type, artwork: nil, artist: first.subtitle)
        return [INPlayMediaMediaItemResolutionResult(mediaItemResolutionResult: .success(with: item))]
    }

    func handle(intent: INPlayMediaIntent) async -> INPlayMediaIntentResponse {
        guard let ident = intent.mediaItems?.first?.identifier,
              let bar = ident.firstIndex(of: "|") else {
            return INPlayMediaIntentResponse(code: .failureUnknownMediaType, userActivity: nil)
        }
        let kind = String(ident[..<bar])
        let id = String(ident[ident.index(after: bar)...])
        let ok = await Library.play(kind, id, shuffle: intent.playShuffled ?? false)
        return INPlayMediaIntentResponse(code: ok ? .success : .failure, userActivity: nil)
    }
}
