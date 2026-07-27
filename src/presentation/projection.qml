import QtQuick
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: projectionWindow

    // Índice do monitor com fallback seguro para a tela primária.
    readonly property int screenIndex: {
        var idx = appController.projector_screen_index;
        var screens = Qt.application.screens;
        if (idx >= 0 && idx < screens.length)
            return idx;
        return 0;
    }

    flags: Qt.FramelessWindowHint
    color: appController.background_color
    screen: Qt.application.screens[screenIndex]
    visibility: appController.projection_visible ? Window.FullScreen : Window.Hidden

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 32
        spacing: 12

        Item {
            Layout.fillHeight: true
        }

        Text {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: appController.clear_screen ? "" : appController.lyric_text.toUpperCase()
            color: appController.font_color
            font.pixelSize: appController.font_size
            font.family: appController.font_family
        }

        Text {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: appController.clear_screen ? "" : appController.next_lyric_text.toUpperCase()
            color: "#D0D0D0"
            opacity: 0.72
            font.pixelSize: Math.max(12, Math.round(appController.font_size * 0.6))
            font.family: appController.font_family
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
