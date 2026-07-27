import QtQuick
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

    Text {
        anchors.fill: parent
        anchors.margins: 32
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.WordWrap
        text: appController.lyric_text
        color: appController.font_color
        font.pixelSize: appController.font_size
        font.family: appController.font_family
    }
}
