import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import QtQuick.Window

// Componentes locais reutilizáveis ficam em arquivos QML separados.

ApplicationWindow {
    id: operatorWindow
    visible: true
    width: 1280
    height: 720
    minimumWidth: 560
    minimumHeight: 420
    title: "Letras Sync - SoringDev"
    font.family: "Inter"

    readonly property string uiFontFamily: "Inter"
    readonly property string uiFontFallback: "Sans"
    readonly property color uiBg: "#121214"
    readonly property color uiCard: "#1E1E24"
    readonly property color uiCardAlt: "#17171C"
    readonly property color uiDivider: "#2E303C"
    readonly property color uiText: "#FFFFFF"
    readonly property color uiTextMuted: "#A0A4B8"
    readonly property color uiAccent: "#3B82F6"
    readonly property color uiAccentPressed: "#2563EB"
    readonly property color uiAccentSoft: "#2D3244"
    readonly property color uiDanger: "#DC2626"
    readonly property color uiSuccess: "#22C55E"

    readonly property var titleFont: Qt.font({ family: uiFontFamily, pixelSize: 32, weight: Font.Bold })
    readonly property var headerFont: Qt.font({ family: uiFontFamily, pixelSize: 22, weight: Font.Bold })
    readonly property var labelFont: Qt.font({ family: uiFontFamily, pixelSize: 24, weight: Font.Medium })
    readonly property var inputFont: Qt.font({ family: uiFontFamily, pixelSize: 24 })
    readonly property var bodyFont: Qt.font({ family: uiFontFamily, pixelSize: 24 })
    readonly property var smallFont: Qt.font({ family: uiFontFamily, pixelSize: 18 })
    readonly property int styleInputWidth: 320
    readonly property int styleLabelWidth: 260

    // A paleta cascateia para todos os controles padrão (Button, TextField,
    // ComboBox, SpinBox, CheckBox, Slider...) sem precisar restilizar cada
    // um individualmente — menos código customizado, visual mais coeso.
    palette.window: uiBg
    palette.windowText: uiText
    palette.base: uiCardAlt
    palette.alternateBase: uiCard
    palette.button: uiCard
    palette.buttonText: uiText
    palette.text: uiText
    palette.brightText: uiText
    palette.highlight: uiAccent
    palette.highlightedText: uiText
    palette.mid: uiDivider
    palette.midlight: uiDivider
    palette.dark: uiDivider
    palette.placeholderText: uiTextMuted

    background: Rectangle {
        color: operatorWindow.uiBg
    }

    property string lyricDraft: ""

    Component.onCompleted: {
        appController.refresh_history()
        lyricDraft = appController.lyric_text
    }

    Connections {
        target: appController

        function onLyric_textChanged() {
            if (!lyricEditField.activeFocus)
                operatorWindow.lyricDraft = appController.lyric_text
        }

        function onActive_line_idChanged() {
            if (!lyricEditField.activeFocus)
                operatorWindow.lyricDraft = appController.lyric_text
        }

        function onLouvorja_search_resultsChanged() {
            if (appController.louvorja_search_results.length > 0)
                louvorjaSearchDialog.open()
            else
                louvorjaSearchDialog.close()
        }
    }

    function saveLyricEdit() {
        if (appController.active_line_id < 0)
            return;
        appController.update_lyric_line(appController.active_line_id, lyricDraft)
    }

    function loadMusic() {
        if (urlField.text.length > 0)
            appController.load_music(urlField.text)
    }

    function exportFormatFromFilter(filterName) {
        return filterName.indexOf("SRT") >= 0 ? "srt" : "lrc"
    }

    function formatTime(seconds) {
        if (isNaN(seconds) || seconds < 0)
            return "0:00";
        var total = Math.floor(seconds);
        var mins = Math.floor(total / 60);
        var secs = total % 60;
        return mins + ":" + (secs < 10 ? "0" + secs : secs);
    }

    function parseNumber(text, fallback) {
        var value = parseFloat(text);
        return isNaN(value) ? fallback : value;
    }

    readonly property bool hasMusic: appController.playback_state === "Playing"
        || appController.playback_state === "Paused"

    FileDialog {
        id: importLyricsDialog
        title: "Importar letras"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Letras sincronizadas (*.lrc *.srt *.vtt)", "LRC (*.lrc)", "SRT (*.srt)", "WebVTT (*.vtt)"]
        onAccepted: appController.import_lyrics(appController.current_music_id, selectedFile.toString())
    }

    FileDialog {
        id: exportLyricsDialog
        title: "Exportar letras"
        fileMode: FileDialog.SaveFile
        nameFilters: ["LRC (*.lrc)", "SRT (*.srt)"]
        onAccepted: appController.export_lyrics(
            appController.current_music_id,
            selectedFile.toString(),
            operatorWindow.exportFormatFromFilter(selectedNameFilter.name)
        )
    }

    FileDialog {
        id: localAudioDialog
        title: "Abrir arquivo de áudio"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Áudio (*.mp3 *.wav *.m4a *.ogg *.mp4)"]
        onAccepted: {
            urlField.text = selectedFile.toString()
            appController.load_music(urlField.text)
        }
    }

    Dialog {
        id: louvorjaSearchDialog
        modal: true
        focus: true
        x: Math.round((operatorWindow.width - width) / 2)
        y: Math.round((operatorWindow.height - height) / 2)
        width: Math.min(operatorWindow.width * 0.88, 560)
        height: Math.min(operatorWindow.height * 0.82, 460)
        padding: 16
        standardButtons: Dialog.NoButton

        background: Rectangle {
            color: operatorWindow.uiCard
            radius: 10
        }

        function loadLouvorjaSong(id) {
            appController.load_louvorja_song(id)
            louvorjaSearchDialog.close()
        }

        contentItem: ColumnLayout {
            spacing: 14

            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        Layout.fillWidth: true
                        text: "Músicas encontradas no sistema do LouvorJA"
                        color: operatorWindow.uiText
                        font: operatorWindow.headerFont
                        elide: Text.ElideRight
                    }

                    Label {
                        Layout.fillWidth: true
                        text: "Selecione a versão desejada para carregar o áudio e a letra oficial"
                        color: operatorWindow.uiTextMuted
                        font: operatorWindow.labelFont
                        wrapMode: Text.WordWrap
                    }
                }

                IconButton {
                    text: "✕"
                    tip: "Fechar"
                    onClicked: louvorjaSearchDialog.close()
                }
            }

            ListView {
                id: louvorjaResultsList
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6
                model: appController.louvorja_search_results

                delegate: Rectangle {
                    id: resultCard
                    width: ListView.view.width
                    implicitHeight: resultRow.implicitHeight + 14
                    radius: 8
                    color: cardMouseArea.containsMouse ? "#1C1F29" : "transparent"

                    property bool pendingSingleLoad: false

                    Timer {
                        id: singleClickTimer
                        interval: 220
                        repeat: false
                        onTriggered: {
                            if (resultCard.pendingSingleLoad)
                                louvorjaSearchDialog.loadLouvorjaSong(modelData.id)
                            resultCard.pendingSingleLoad = false
                        }
                    }

                    MouseArea {
                        id: cardMouseArea
                        anchors.fill: parent
                        z: -1
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton
                        onClicked: {
                            resultCard.pendingSingleLoad = true
                            singleClickTimer.restart()
                        }
                        onDoubleClicked: {
                            singleClickTimer.stop()
                            resultCard.pendingSingleLoad = false
                            louvorjaSearchDialog.loadLouvorjaSong(modelData.id)
                        }
                    }

                    RowLayout {
                        id: resultRow
                        anchors.fill: parent
                        anchors.margins: 7
                        spacing: 12

                        Pill {
                            text: "ID " + modelData.id
                        }

                        Label {
                            Layout.fillWidth: true
                            text: modelData.name
                            color: operatorWindow.uiText
                            font: operatorWindow.headerFont
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                        }

                        Label {
                            text: modelData.album.length > 0
                                ? modelData.album
                                : "Álbum não informado"
                            color: operatorWindow.uiTextMuted
                            font: operatorWindow.labelFont
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                        }

                        AppButton {
                            text: "Carregar"
                            variant: "primary"
                            onClicked: louvorjaSearchDialog.loadLouvorjaSong(modelData.id)
                        }
                    }
                }
            }
        }
    }

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
        onActivated: appController.set_font_size(Math.min(appController.font_size + 2, 150))
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

    // Painel de histórico, agora sob demanda em vez de ocupar metade da
    // tela o tempo todo — libera espaço para a letra, que é o que importa
    // durante a operação ao vivo.
    Drawer {
        id: historyDrawer
        edge: Qt.RightEdge
        width: Math.min(360, operatorWindow.width * 0.86)
        height: operatorWindow.height
        modal: true
        dim: true

        background: Rectangle {
            color: operatorWindow.uiCardAlt
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                Eyebrow { Layout.fillWidth: true; text: "HISTÓRICO" }
                IconButton { text: "✕"; tip: "Fechar"; onClicked: historyDrawer.close() }
            }

            TextField {
                Layout.fillWidth: true
                placeholderText: "Filtrar histórico..."
                font: operatorWindow.inputFont
                selectByMouse: true
                onTextChanged: appController.set_history_search_query(text)
            }

            Label {
                Layout.fillWidth: true
                text: "Nenhuma música no histórico"
                color: operatorWindow.uiTextMuted
                font: operatorWindow.labelFont
                visible: appController.history.length === 0
            }

            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 14
                visible: appController.history.length > 0
                model: appController.history

                delegate: RowLayout {
                    width: ListView.view.width
                    spacing: 6

                    IconButton { text: "▶"; tip: "Carregar"; onClicked: appController.load_music(modelData.youtube_url) }
                    IconButton { text: "+"; tip: "Adicionar à fila"; onClicked: appController.add_to_playlist(modelData.youtube_url) }
                    IconButton {
                        text: "✕"
                        tip: "Limpar letra e recarregar"
                        idleColor: "#DC6A6A"
                        hoverColor: "#F87171"
                        onClicked: {
                            appController.clear_lyrics(modelData.youtube_url)
                            appController.load_music(modelData.youtube_url)
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 3

                        Label {
                            Layout.fillWidth: true
                            text: modelData.title
                            color: operatorWindow.uiText
                            font: operatorWindow.headerFont
                            elide: Text.ElideRight
                        }

                        RowLayout {
                            spacing: 6
                            Pill {
                                text: modelData.has_lyrics ? "Letra salva" : "Pendente"
                                tint: modelData.has_lyrics ? "#16321F" : "#242935"
                                textColor: modelData.has_lyrics ? "#4ADE80" : "#A0A4B8"
                            }
                            Label {
                                Layout.fillWidth: true
                                text: modelData.artist
                                font: operatorWindow.labelFont
                                color: operatorWindow.uiTextMuted
                                visible: modelData.artist.length > 0
                                elide: Text.ElideRight
                            }
                        }
                    }
                }
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            TabBar {
                id: tabBar
                Layout.fillWidth: true
                font: operatorWindow.labelFont
                background: Item {}

                TabButton {
                    text: "Player"
                    font: operatorWindow.labelFont
                    background: Rectangle {
                        color: "transparent"
                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: 2
                            color: tabBar.currentIndex === 0 ? operatorWindow.uiAccent : "transparent"
                        }
                    }
                    contentItem: Text {
                        text: parent.text
                        color: tabBar.currentIndex === 0 ? operatorWindow.uiText : operatorWindow.uiTextMuted
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        font: operatorWindow.labelFont
                    }
                }

                TabButton {
                    text: "Playlist"
                    font: operatorWindow.labelFont
                    background: Rectangle {
                        color: "transparent"
                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: 2
                            color: tabBar.currentIndex === 1 ? operatorWindow.uiAccent : "transparent"
                        }
                    }
                    contentItem: Text {
                        text: parent.text
                        color: tabBar.currentIndex === 1 ? operatorWindow.uiText : operatorWindow.uiTextMuted
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        font: operatorWindow.labelFont
                    }
                }

                TabButton {
                    text: "Estilo"
                    font: operatorWindow.labelFont
                    background: Rectangle {
                        color: "transparent"
                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: 2
                            color: tabBar.currentIndex === 2 ? operatorWindow.uiAccent : "transparent"
                        }
                    }
                    contentItem: Text {
                        text: parent.text
                        color: tabBar.currentIndex === 2 ? operatorWindow.uiText : operatorWindow.uiTextMuted
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        font: operatorWindow.labelFont
                    }
                }
            }

            AppButton {
                text: "Histórico"
                variant: "ghost"
                onClicked: historyDrawer.open()
            }
        }

        Divider {}

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabBar.currentIndex

            // ------------------------------------------------------------
            // Aba "Player"
            // ------------------------------------------------------------
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 14

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            TextField {
                                id: urlField
                                Layout.fillWidth: true
                                placeholderText: "Cole a URL do YouTube ou digite o nome da música"
                                selectByMouse: true
                                font: operatorWindow.inputFont
                                onAccepted: operatorWindow.loadMusic()
                            }

                            AppButton {
                                text: "Carregar"
                                variant: "primary"
                                enabled: urlField.text.length > 0 && !appController.loading
                                onClicked: operatorWindow.loadMusic()
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            AppButton {
                                text: "Arquivo local"
                                variant: "ghost"
                                enabled: !appController.loading
                                onClicked: localAudioDialog.open()
                            }

                            AppButton {
                                text: "Buscar no LouvorJA"
                                variant: "ghost"
                                enabled: urlField.text.length > 0 && !appController.loading
                                onClicked: appController.search_louvorja(urlField.text)
                            }

                            Item { Layout.fillWidth: true }

                            CheckBox {
                                text: "AutoPlay"
                                checked: appController.autoplay
                                onClicked: appController.set_autoplay(checked)
                                font: operatorWindow.smallFont
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 6

                            Label {
                                Layout.fillWidth: true
                                text: "Depuração de provider"
                                color: operatorWindow.uiTextMuted
                                font: operatorWindow.smallFont
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 12

                                CheckBox {
                                    text: "LRCLib"
                                    checked: appController.debug_lyrics_provider_override === "lrclib"
                                    onClicked: appController.set_debug_lyrics_provider_override(checked ? "lrclib" : "")
                                    font: operatorWindow.smallFont
                                }

                                CheckBox {
                                    text: "YouTube"
                                    checked: appController.debug_lyrics_provider_override === "youtube"
                                    onClicked: appController.set_debug_lyrics_provider_override(checked ? "youtube" : "")
                                    font: operatorWindow.smallFont
                                }

                                CheckBox {
                                    text: "NetEase"
                                    checked: appController.debug_lyrics_provider_override === "netease"
                                    onClicked: appController.set_debug_lyrics_provider_override(checked ? "netease" : "")
                                    font: operatorWindow.smallFont
                                }

                                CheckBox {
                                    text: "Whisper"
                                    checked: appController.debug_lyrics_provider_override === "whisper"
                                    onClicked: appController.set_debug_lyrics_provider_override(checked ? "whisper" : "")
                                    font: operatorWindow.smallFont
                                }

                                Item { Layout.fillWidth: true }
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        visible: appController.loading

                        BusyIndicator {
                            running: appController.loading
                            implicitWidth: 32
                            implicitHeight: 32
                            Layout.preferredWidth: 32
                            Layout.preferredHeight: 32
                        }

                        Label {
                            text: appController.loading_status.length > 0
                                ? appController.loading_status
                                : "Carregando..."
                            color: operatorWindow.uiTextMuted
                            font: operatorWindow.labelFont
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        text: appController.error_message
                        color: appController.error_message.startsWith("OK:")
                            ? operatorWindow.uiSuccess
                            : operatorWindow.uiDanger
                        font: operatorWindow.labelFont
                        wrapMode: Text.WordWrap
                        visible: appController.error_message.length > 0
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Rectangle {
                            Layout.preferredWidth: 40
                            Layout.preferredHeight: 40
                            radius: 10
                            color: "#172033"

                            Text {
                                anchors.centerIn: parent
                                text: "♫"
                                color: operatorWindow.uiAccent
                                font.pixelSize: 22
                                font.weight: Font.Bold
                            }
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 1

                            Label {
                                Layout.fillWidth: true
                                text: appController.music_title.length > 0
                                    ? appController.music_title
                                    : "Nenhuma música carregada"
                                color: operatorWindow.uiText
                                font: operatorWindow.headerFont
                                elide: Text.ElideRight
                            }

                            Label {
                                Layout.fillWidth: true
                                text: appController.music_artist
                                color: operatorWindow.uiTextMuted
                                font: operatorWindow.smallFont
                                visible: appController.music_artist.length > 0
                                elide: Text.ElideRight
                            }
                        }

                        AppButton {
                            text: appController.projection_visible ? "OCULTAR" : "PROJETAR"
                            variant: appController.projection_visible ? "primary" : "secondary"
                            textPixelSize: 12
                            padding: 6
                            leftPadding: 12
                            rightPadding: 12
                            onClicked: appController.toggle_projection()
                        }
                    }

                    Divider {}

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        visible: appController.active_line_id >= 0

                        TextField {
                            id: lyricEditField
                            Layout.fillWidth: true
                            placeholderText: "Corrigir a letra ativa..."
                            selectByMouse: true
                            text: operatorWindow.lyricDraft
                            font: operatorWindow.inputFont
                            onTextEdited: operatorWindow.lyricDraft = text
                            onAccepted: operatorWindow.saveLyricEdit()
                        }

                        AppButton {
                            text: "SALVAR"
                            variant: "primary"
                            onClicked: operatorWindow.saveLyricEdit()
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 8
                        visible: appController.current_lyrics.length > 0

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Eyebrow { Layout.fillWidth: true; text: "TODAS AS LINHAS" }

                            AppButton {
                                text: "IMPORTAR"
                                variant: "ghost"
                                enabled: appController.current_music_id.length > 0 && !appController.loading
                                onClicked: importLyricsDialog.open()
                            }

                            AppButton {
                                text: "EXPORTAR"
                                variant: "ghost"
                                enabled: appController.current_music_id.length > 0 && !appController.loading
                                onClicked: exportLyricsDialog.open()
                            }
                        }

                        ListView {
                            id: lyricsListView
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            spacing: 4
                            model: appController.current_lyrics
                            currentIndex: {
                                for (var i = 0; i < appController.current_lyrics.length; i++) {
                                    if (appController.current_lyrics[i].id === appController.active_line_id)
                                        return i;
                                }
                                return -1;
                            }

                            onCurrentIndexChanged: {
                                if (currentIndex >= 0)
                                    positionViewAtIndex(currentIndex, ListView.Center)
                            }

                            delegate: Rectangle {
                                width: ListView.view.width
                                height: Math.max(32, lineText.implicitHeight + 12)
                                radius: 6
                                color: modelData.id === appController.active_line_id
                                    ? operatorWindow.uiAccentSoft
                                    : "transparent"

                                Rectangle {
                                    width: modelData.id === appController.active_line_id ? 3 : 0
                                    anchors.left: parent.left
                                    anchors.top: parent.top
                                    anchors.bottom: parent.bottom
                                    color: operatorWindow.uiAccent
                                    visible: modelData.id === appController.active_line_id
                                }

                                Text {
                                    id: lineText
                                    anchors.fill: parent
                                    anchors.leftMargin: 10
                                    anchors.rightMargin: 8
                                    anchors.topMargin: 6
                                    anchors.bottomMargin: 6
                                    text: modelData.text
                                    wrapMode: Text.WordWrap
                                    color: modelData.id === appController.active_line_id
                                        ? operatorWindow.uiText
                                        : operatorWindow.uiTextMuted
                                    font: modelData.id === appController.active_line_id
                                        ? operatorWindow.headerFont
                                        : operatorWindow.labelFont
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: appController.seek(modelData.start_time)
                                }
                            }
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Item { Layout.fillWidth: true }
                        AppButton {
                            text: "LIMPAR DB"
                            variant: "danger"
                            enabled: !appController.loading
                            onClicked: appController.clear_database()
                        }
                    }

                    Divider {}

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            Label {
                                text: operatorWindow.formatTime(appController.current_time)
                                color: operatorWindow.uiTextMuted
                                font: operatorWindow.labelFont
                            }

                            Slider {
                                id: progressSlider
                                Layout.fillWidth: true
                                enabled: operatorWindow.hasMusic && appController.duration > 0
                                from: 0
                                to: appController.duration > 0 ? appController.duration : 1

                                // Só espelha o tempo do player quando o usuário não está
                                // arrastando, evitando loop de seek.
                                value: pressed ? value : appController.current_time

                                onMoved: {
                                    if (appController.duration > 0)
                                        appController.seek(value)
                                }
                                onPressedChanged: {
                                    if (!pressed && appController.duration > 0)
                                        appController.seek(value)
                                }
                            }

                            Label {
                                text: operatorWindow.formatTime(appController.duration)
                                color: operatorWindow.uiTextMuted
                                font: operatorWindow.labelFont
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 4

                            IconButton { text: "⏮"; tip: "Anterior"; iconPixelSize: 26; onClicked: appController.play_previous() }

                            IconButton {
                                text: appController.playback_state === "Playing" ? "⏸" : "▶"
                                tip: appController.playback_state === "Playing" ? "Pausar" : "Reproduzir"
                                iconPixelSize: 26
                                enabled: operatorWindow.hasMusic
                                idleColor: operatorWindow.uiAccent
                                hoverColor: operatorWindow.uiAccent
                                onClicked: appController.playback_state === "Playing"
                                    ? appController.pause()
                                    : appController.play()
                            }

                            IconButton { text: "⏹"; tip: "Parar"; iconPixelSize: 26; enabled: operatorWindow.hasMusic; onClicked: appController.stop() }
                            IconButton { text: "⏭"; tip: "Próxima"; iconPixelSize: 26; onClicked: appController.play_next() }
                            IconButton { text: "−10s"; enabled: operatorWindow.hasMusic; onClicked: appController.seek_relative(-10.0) }
                            IconButton { text: "+10s"; enabled: operatorWindow.hasMusic; onClicked: appController.seek_relative(10.0) }

                            Item { Layout.fillWidth: true }

                            AppButton {
                                text: appController.clear_screen ? "MOSTRAR TEXTO" : "OCULTAR TEXTO"
                                variant: appController.clear_screen ? "primary" : "secondary"
                                enabled: operatorWindow.hasMusic || appController.clear_screen
                                onClicked: appController.toggle_clear_screen()
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Label { text: "🔊"; color: operatorWindow.uiTextMuted; font: operatorWindow.smallFont }

                            Slider {
                                Layout.preferredWidth: 110
                                from: 0
                                to: 100
                                stepSize: 1
                                value: appController.volume
                                onMoved: appController.set_volume(Math.round(value))
                            }

                            Label {
                                text: Math.round(appController.volume) + "%"
                                color: operatorWindow.uiTextMuted
                                font: operatorWindow.labelFont
                            }

                            Rectangle {
                                width: 1
                                height: 22
                                color: operatorWindow.uiDivider
                            }

                            IconButton { text: "−0.5s"; tip: "Atrasar letra"; onClicked: appController.adjust_sync_offset(-0.5) }
                            IconButton { text: "+0.5s"; tip: "Adiantar letra"; onClicked: appController.adjust_sync_offset(0.5) }

                            Label {
                                Layout.fillWidth: true
                                text: "Sincronismo "
                                    + (appController.sync_offset >= 0 ? "+" : "")
                                    + appController.sync_offset.toFixed(1) + "s"
                                horizontalAlignment: Text.AlignRight
                                color: operatorWindow.uiTextMuted
                                font: operatorWindow.labelFont
                                elide: Text.ElideRight
                            }
                        }
                    }
                }
            }

            // ------------------------------------------------------------
            // Aba "Playlist"
            // ------------------------------------------------------------
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 12

                Eyebrow { text: "FILA DE REPRODUÇÃO" }

                Label {
                    Layout.fillWidth: true
                    text: "Nenhuma música na fila"
                    color: operatorWindow.uiTextMuted
                    font: operatorWindow.labelFont
                    visible: appController.playlist.length === 0
                }

                ListView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 14
                    visible: appController.playlist.length > 0
                    model: appController.playlist

                    delegate: RowLayout {
                        width: ListView.view.width
                        spacing: 6

                        IconButton { text: "▶"; tip: "Reproduzir agora"; onClicked: appController.play_playlist_item(index) }
                        IconButton { text: "✕"; tip: "Remover da fila"; idleColor: "#DC6A6A"; hoverColor: "#F87171"; onClicked: appController.remove_from_playlist(index) }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 3

                            Label {
                                Layout.fillWidth: true
                                text: modelData.title
                                color: operatorWindow.uiText
                                font: operatorWindow.headerFont
                                elide: Text.ElideRight
                            }

                            RowLayout {
                                spacing: 6
                                Pill {
                                    text: modelData.has_lyrics ? "Letra salva" : "Pendente"
                                    tint: modelData.has_lyrics ? "#16321F" : "#242935"
                                    textColor: modelData.has_lyrics ? "#4ADE80" : "#A0A4B8"
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: modelData.artist
                                    font: operatorWindow.labelFont
                                    color: operatorWindow.uiTextMuted
                                    visible: modelData.artist.length > 0
                                    elide: Text.ElideRight
                                }
                            }
                        }
                    }
                }
            }

            // ------------------------------------------------------------
            // Aba "Estilo" — agrupada por seção e rolável, para não cortar
            // controles em janelas menores.
            // ------------------------------------------------------------
            ScrollView {
                id: styleScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true

                ColumnLayout {
                    width: styleScroll.availableWidth
                    spacing: 22

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Eyebrow { text: "TEXTO" }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Tamanho da fonte"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            SpinBox {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                font: operatorWindow.inputFont
                                from: 12; to: 150; value: appController.font_size
                                onValueModified: appController.set_font_size(value)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Cor do texto"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            TextField {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                text: appController.font_color
                                font: operatorWindow.inputFont
                                onEditingFinished: appController.set_font_color(text)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Cor de fundo"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            TextField {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                text: appController.background_color
                                font: operatorWindow.inputFont
                                onEditingFinished: appController.set_background_color(text)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Família da fonte"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            TextField {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                text: appController.font_family
                                font: operatorWindow.inputFont
                                onEditingFinished: appController.set_font_family(text)
                            }
                        }
                    }

                    Divider {}

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Eyebrow { text: "LAYOUT DA PROJEÇÃO" }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Peso da fonte"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            SpinBox {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                font: operatorWindow.inputFont
                                from: 100; to: 900; stepSize: 100
                                value: appController.projection_font_weight
                                onValueModified: appController.set_projection_font_weight(value)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Espaçamento"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            TextField {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                text: appController.projection_letter_spacing.toString()
                                font: operatorWindow.inputFont
                                onEditingFinished: appController.set_projection_letter_spacing(
                                    operatorWindow.parseNumber(text, appController.projection_letter_spacing)
                                )
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Altura da linha"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            TextField {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                text: appController.projection_line_height_multiplier.toString()
                                font: operatorWindow.inputFont
                                onEditingFinished: appController.set_projection_line_height_multiplier(
                                    operatorWindow.parseNumber(text, appController.projection_line_height_multiplier)
                                )
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Margem horizontal"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            SpinBox {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                font: operatorWindow.inputFont
                                from: 0; to: 500; value: appController.projection_margin_horizontal
                                onValueModified: appController.set_projection_margin_horizontal(value)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Margem vertical"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            SpinBox {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                font: operatorWindow.inputFont
                                from: 0; to: 300; value: appController.projection_margin_vertical
                                onValueModified: appController.set_projection_margin_vertical(value)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "A. Horizontal"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            ComboBox {
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                model: ["center", "left", "right"]
                                currentIndex: appController.projection_horizontal_alignment === "left"
                                    ? 1
                                    : appController.projection_horizontal_alignment === "right"
                                        ? 2
                                        : 0
                                onActivated: appController.set_projection_horizontal_alignment(currentText)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "A. Vertical"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            ComboBox {
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                model: ["center", "top", "bottom"]
                                currentIndex: appController.projection_vertical_alignment === "top"
                                    ? 1
                                    : appController.projection_vertical_alignment === "bottom"
                                        ? 2
                                        : 0
                                onActivated: appController.set_projection_vertical_alignment(currentText)
                            }
                        }
                    }

                    Divider {}

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Eyebrow { text: "SOMBRA" }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Ativar sombra"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            CheckBox {
                                checked: appController.projection_shadow_enabled
                                font: operatorWindow.inputFont
                                leftPadding: 0
                                spacing: 12
                                indicator: Rectangle {
                                    implicitWidth: 22
                                    implicitHeight: 22
                                    x: 0
                                    y: parent.height / 2 - height / 2
                                    radius: 5
                                    color: appController.projection_shadow_enabled ? operatorWindow.uiAccent : operatorWindow.uiCardAlt
                                    border.width: 1
                                    border.color: appController.projection_shadow_enabled ? operatorWindow.uiAccent : operatorWindow.uiDivider

                                    Rectangle {
                                        anchors.centerIn: parent
                                        width: 10
                                        height: 10
                                        radius: 2
                                        color: operatorWindow.uiText
                                        visible: appController.projection_shadow_enabled
                                    }
                                }
                                onToggled: appController.set_projection_shadow_enabled(checked)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Cor da sombra"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            TextField {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                text: appController.projection_shadow_color
                                font: operatorWindow.inputFont
                                onEditingFinished: appController.set_projection_shadow_color(text)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Offset da sombra X"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            SpinBox {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                font: operatorWindow.inputFont
                                from: -100; to: 100; value: appController.projection_shadow_offset_x
                                onValueModified: appController.set_projection_shadow_offset_x(value)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Offset da sombra Y"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            SpinBox {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                font: operatorWindow.inputFont
                                from: -100; to: 100; value: appController.projection_shadow_offset_y
                                onValueModified: appController.set_projection_shadow_offset_y(value)
                            }
                        }
                    }

                    Divider {}

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Eyebrow { text: "ANIMAÇÃO E MONITOR" }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Duração do fade (ms)"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            SpinBox {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                font: operatorWindow.inputFont
                                from: 0; to: 5000; stepSize: 10
                                value: appController.projection_fade_duration_ms
                                onValueModified: appController.set_projection_fade_duration_ms(value)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Animação de fade"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont }
                            CheckBox {
                                checked: appController.projection_fade_animation_enabled
                                font: operatorWindow.inputFont
                                leftPadding: 0
                                spacing: 12
                                indicator: Rectangle {
                                    implicitWidth: 22
                                    implicitHeight: 22
                                    x: 0
                                    y: parent.height / 2 - height / 2
                                    radius: 5
                                    color: appController.projection_fade_animation_enabled ? operatorWindow.uiAccent : operatorWindow.uiCardAlt
                                    border.width: 1
                                    border.color: appController.projection_fade_animation_enabled ? operatorWindow.uiAccent : operatorWindow.uiDivider

                                    Rectangle {
                                        anchors.centerIn: parent
                                        width: 10
                                        height: 10
                                        radius: 2
                                        color: operatorWindow.uiText
                                        visible: appController.projection_fade_animation_enabled
                                    }
                                }
                                onToggled: appController.set_projection_fade_animation_enabled(checked)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            Label { Layout.preferredWidth: operatorWindow.styleLabelWidth; text: "Monitor do projetor (-1 = primário)"; color: operatorWindow.uiTextMuted; font: operatorWindow.labelFont; wrapMode: Text.WordWrap }
                            SpinBox {
                                Layout.fillWidth: true
                                Layout.preferredWidth: operatorWindow.styleInputWidth
                                Layout.maximumWidth: operatorWindow.styleInputWidth
                                font: operatorWindow.inputFont
                                from: -1; to: 9; value: appController.projector_screen_index
                                onValueModified: appController.set_projector_screen_index(value)
                            }
                        }
                    }

                    Divider {}

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Eyebrow { text: "PRÉVIA" }

                        Rectangle {
                            Layout.fillWidth: true
                            height: 90
                            color: operatorWindow.uiCardAlt
                            radius: 10

                            Text {
                                anchors.centerIn: parent
                                anchors.margins: 12
                                width: parent.width - 24
                                text: appController.lyric_text.length > 0
                                    ? appController.lyric_text
                                    : "Grande é o Senhor"
                                color: operatorWindow.uiText
                                font.pixelSize: 14
                                font.family: operatorWindow.uiFontFamily
                                font.weight: appController.projection_font_weight
                                font.letterSpacing: appController.projection_letter_spacing
                                horizontalAlignment: Text.AlignHCenter
                                lineHeightMode: Text.ProportionalHeight
                                lineHeight: 1.25
                                wrapMode: Text.WordWrap
                            }
                        }
                    }

                    Item { Layout.preferredHeight: 8 }
                }
            }
        }
    }
}
