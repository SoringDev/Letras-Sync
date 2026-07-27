import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

ApplicationWindow {
    id: operatorWindow
    visible: true
    width: 640
    height: 480
    title: "Letras Sync"

    Component.onCompleted: appController.refresh_history()

    Shortcut {
        sequence: "Space"
        context: Qt.WindowShortcut
        onActivated: appController.toggle_clear_screen()
    }

    Shortcut {
        sequence: "Left"
        context: Qt.WindowShortcut
        onActivated: appController.seek_relative(-10.0)
    }

    Shortcut {
        sequence: "Right"
        context: Qt.WindowShortcut
        onActivated: appController.seek_relative(10.0)
    }

    Shortcut {
        sequence: "Up"
        context: Qt.WindowShortcut
        onActivated: appController.set_volume(Math.min(appController.volume + 5, 100))
    }

    Shortcut {
        sequence: "Down"
        context: Qt.WindowShortcut
        onActivated: appController.set_volume(Math.max(appController.volume - 5, 0))
    }

    Shortcut {
        sequence: "PageDown"
        context: Qt.WindowShortcut
        onActivated: appController.play_next()
    }

    Shortcut {
        sequence: "PageUp"
        context: Qt.WindowShortcut
        onActivated: appController.play_previous()
    }

    Shortcut {
        sequence: "Esc"
        context: Qt.WindowShortcut
        onActivated: appController.stop()
    }

    Shortcut {
        sequence: "Ctrl+="
        context: Qt.WindowShortcut
        onActivated: appController.set_font_size(appController.font_size + 2)
    }

    Shortcut {
        sequence: "Ctrl+-"
        context: Qt.WindowShortcut
        onActivated: appController.set_font_size(Math.max(appController.font_size - 2, 12))
    }

    // Janela de projeção controlada pelo mesmo appController.
    Loader {
        source: "qrc:/letras_sync/presentation/projection.qml"
    }

    readonly property bool hasMusic: appController.playback_state === "Playing"
        || appController.playback_state === "Paused"

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 12

        TabBar {
            id: tabBar
            Layout.fillWidth: true

            TabButton {
                text: "Operação"
            }

            TabButton {
                text: "Playlist"
            }

            TabButton {
                text: "Estilo"
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabBar.currentIndex

            // Aba "Operação"
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 12

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    TextField {
                        id: urlField
                        Layout.fillWidth: true
                        placeholderText: "Cole a URL do YouTube"
                        selectByMouse: true
                    }

                    Button {
                        text: "Carregar"
                        enabled: urlField.text.length > 0 && !appController.loading
                        onClicked: appController.load_music(urlField.text)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    visible: appController.loading

                    BusyIndicator {
                        running: appController.loading
                        implicitWidth: 24
                        implicitHeight: 24
                    }

                    Label {
                        text: appController.loading_status.length > 0
                            ? appController.loading_status
                            : "Carregando..."
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: appController.error_message
                    color: "#CC0000"
                    wrapMode: Text.WordWrap
                    visible: appController.error_message.length > 0
                }

                Label {
                    Layout.fillWidth: true
                    text: appController.music_title.length > 0
                        ? appController.music_title
                        : "Nenhuma música carregada"
                    font.pixelSize: 20
                    font.bold: true
                    elide: Text.ElideRight
                }

                Label {
                    Layout.fillWidth: true
                    text: appController.music_artist
                    font.pixelSize: 14
                    color: "#666666"
                    visible: appController.music_artist.length > 0
                    elide: Text.ElideRight
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        text: "Letras"
                        font.bold: true
                    }

                    Label {
                        Layout.fillWidth: true
                        text: "Nenhuma letra carregada"
                        color: "#666666"
                        visible: appController.current_lyrics.length === 0
                    }

                    ListView {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 160
                        clip: true
                        spacing: 4
                        visible: appController.current_lyrics.length > 0
                        model: appController.current_lyrics

                        delegate: Rectangle {
                            width: ListView.view.width
                            height: Math.max(32, lineText.implicitHeight + 14)
                            radius: 4
                            color: modelData.id === appController.active_line_id
                                ? "#FFF2B8"
                                : "#F7F7F7"
                            border.width: 1
                            border.color: modelData.id === appController.active_line_id
                                ? "#D6A100"
                                : "#DDDDDD"

                            Text {
                                id: lineText
                                anchors.fill: parent
                                anchors.margins: 7
                                text: modelData.text
                                wrapMode: Text.WordWrap
                                font.bold: modelData.id === appController.active_line_id
                                color: "#222222"
                            }

                            MouseArea {
                                anchors.fill: parent
                                onClicked: appController.seek(modelData.start_time)
                            }
                        }
                    }
                }

                Item {
                    Layout.fillHeight: true
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    TextField {
                        Layout.fillWidth: true
                        placeholderText: "Filtrar histórico por título ou artista..."
                        onTextChanged: appController.set_history_search_query(text)
                    }

                    Label {
                        text: "Histórico"
                        font.bold: true
                    }

                    Label {
                        Layout.fillWidth: true
                        text: "Nenhuma música no histórico"
                        color: "#666666"
                        visible: appController.history.length === 0
                    }

                    ListView {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 140
                        clip: true
                        spacing: 4
                        visible: appController.history.length > 0
                        model: appController.history

                        delegate: RowLayout {
                            width: ListView.view.width
                            spacing: 8

                            Button {
                                text: "▶"
                                onClicked: appController.load_music(modelData.youtube_url)
                            }

                            Button {
                                text: "+"
                                onClicked: appController.add_to_playlist(modelData.youtube_url)
                            }

                            Button {
                                text: "✕"
                                onClicked: {
                                    appController.clear_lyrics(modelData.youtube_url)
                                    appController.load_music(modelData.youtube_url)
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 0

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.title
                                    font.bold: true
                                    elide: Text.ElideRight
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.has_lyrics
                                        ? "[Letra Salva]"
                                        : "[Pendente Whisper]"
                                    font.pixelSize: 11
                                    color: modelData.has_lyrics ? "#2E7D32" : "#888888"
                                    elide: Text.ElideRight
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.artist
                                    font.pixelSize: 12
                                    color: "#666666"
                                    visible: modelData.artist.length > 0
                                    elide: Text.ElideRight
                                }
                            }
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Button {
                        text: "⏮"
                        onClicked: appController.play_previous()
                    }

                    Button {
                        text: "Play"
                        enabled: appController.playback_state !== "Playing" && operatorWindow.hasMusic
                        onClicked: appController.play()
                    }

                    Button {
                        text: "Pause"
                        enabled: appController.playback_state === "Playing"
                        onClicked: appController.pause()
                    }

                    Button {
                        text: "Stop"
                        enabled: operatorWindow.hasMusic
                        onClicked: appController.stop()
                    }

                    Button {
                        id: clearScreenButton
                        text: appController.clear_screen ? "Mostrar Texto" : "Ocultar Texto"
                        enabled: operatorWindow.hasMusic || appController.clear_screen
                        onClicked: appController.toggle_clear_screen()
                        background: Rectangle {
                            radius: 4
                            color: appController.clear_screen
                                ? (clearScreenButton.down ? "#B71C1C" : "#D32F2F")
                                : (clearScreenButton.down ? "#D9D9D9" : "#F2F2F2")
                            border.color: appController.clear_screen ? "#8E0000" : "#BDBDBD"
                            border.width: 1
                        }
                        contentItem: Text {
                            text: clearScreenButton.text
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                            color: appController.clear_screen ? "#FFFFFF" : "#222222"
                            font: clearScreenButton.font
                            elide: Text.ElideRight
                        }
                    }

                    Button {
                        text: "⏭"
                        onClicked: appController.play_next()
                    }

                    Button {
                        text: "−10s"
                        enabled: operatorWindow.hasMusic
                        onClicked: appController.seek_relative(-10.0)
                    }

                    Button {
                        text: "+10s"
                        enabled: operatorWindow.hasMusic
                        onClicked: appController.seek_relative(10.0)
                    }

                    Button {
                        text: appController.projection_visible ? "Ocultar" : "Projetar"
                        onClicked: appController.toggle_projection()
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        text: "Volume"
                    }

                    Slider {
                        Layout.fillWidth: true
                        from: 0
                        to: 100
                        stepSize: 1
                        value: appController.volume
                        onMoved: appController.set_volume(Math.round(value))
                    }

                    Label {
                        text: "🔊 " + Math.round(appController.volume) + "%"
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Button {
                        text: "Atrasar Letra (-0.5s)"
                        enabled: operatorWindow.hasMusic
                        onClicked: appController.adjust_sync_offset(-0.5)
                    }

                    Button {
                        text: "Adiantar Letra (+0.5s)"
                        enabled: operatorWindow.hasMusic
                        onClicked: appController.adjust_sync_offset(0.5)
                    }

                    Label {
                        Layout.fillWidth: true
                        text: "Ajuste de Sincronismo: "
                            + (appController.sync_offset >= 0 ? "+" : "")
                            + appController.sync_offset.toFixed(1) + "s"
                        horizontalAlignment: Text.AlignRight
                        elide: Text.ElideRight
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        text: formatTime(appController.current_time)
                    }

                    Slider {
                        id: progressSlider
                        Layout.fillWidth: true
                        from: 0
                        to: appController.duration > 0 ? appController.duration : 1

                        // Só espelha o tempo do player quando o usuário não está
                        // arrastando, evitando loop de seek.
                        value: pressed ? value : appController.current_time

                        onMoved: appController.seek(value)
                    }

                    Label {
                        text: formatTime(appController.duration)
                    }
                }
            }

            // Aba "Playlist"
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 12

                Label {
                    text: "Fila de reprodução"
                    font.bold: true
                }

                Label {
                    Layout.fillWidth: true
                    text: "Nenhuma música na fila"
                    color: "#666666"
                    visible: appController.playlist.length === 0
                }

                ListView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4
                    visible: appController.playlist.length > 0
                    model: appController.playlist

                    delegate: RowLayout {
                        width: ListView.view.width
                        spacing: 8

                        Button {
                            text: "▶"
                            onClicked: appController.play_playlist_item(index)
                        }

                        Button {
                            text: "✕"
                            onClicked: appController.remove_from_playlist(index)
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Label {
                                Layout.fillWidth: true
                                text: modelData.title
                                font.bold: true
                                elide: Text.ElideRight
                            }

                            Label {
                                Layout.fillWidth: true
                                text: modelData.has_lyrics
                                    ? "[Letra Salva]"
                                    : "[Pendente Whisper]"
                                font.pixelSize: 11
                                color: modelData.has_lyrics ? "#2E7D32" : "#888888"
                                elide: Text.ElideRight
                            }

                            Label {
                                Layout.fillWidth: true
                                text: modelData.artist
                                font.pixelSize: 12
                                color: "#666666"
                                visible: modelData.artist.length > 0
                                elide: Text.ElideRight
                            }
                        }
                    }
                }
            }

            // Aba "Estilo"
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 12

                RowLayout {
                    Label { text: "Tamanho da fonte:" }
                    SpinBox {
                        from: 12; to: 120; value: appController.font_size
                        onValueModified: appController.set_font_size(value)
                    }
                }

                RowLayout {
                    Label { text: "Cor do texto:" }
                    TextField {
                        text: appController.font_color
                        onEditingFinished: appController.set_font_color(text)
                    }
                }

                RowLayout {
                    Label { text: "Cor de fundo:" }
                    TextField {
                        text: appController.background_color
                        onEditingFinished: appController.set_background_color(text)
                    }
                }

                RowLayout {
                    Label { text: "Família da fonte:" }
                    TextField {
                        text: appController.font_family
                        onEditingFinished: appController.set_font_family(text)
                    }
                }

                RowLayout {
                    Label { text: "Monitor do projetor (-1 = primário):" }
                    SpinBox {
                        from: -1; to: 9; value: appController.projector_screen_index
                        onValueModified: appController.set_projector_screen_index(value)
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    height: 80
                    color: appController.background_color
                    Text {
                        anchors.centerIn: parent
                        text: appController.lyric_text.length > 0
                            ? appController.lyric_text
                            : "Grande é o Senhor"
                        color: appController.font_color
                        font.pixelSize: Math.min(appController.font_size, 32)
                        font.family: appController.font_family
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                    }
                }

                Item {
                    Layout.fillHeight: true
                }
            }
        }
    }

    function formatTime(seconds) {
        if (isNaN(seconds) || seconds < 0)
            return "0:00";
        var total = Math.floor(seconds);
        var mins = Math.floor(total / 60);
        var secs = total % 60;
        return mins + ":" + (secs < 10 ? "0" + secs : secs);
    }
}
