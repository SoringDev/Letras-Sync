import QtQuick
import QtQuick.Layouts
import QtQuick.Window

Window {
    id: projectionWindow

    readonly property int screenIndex: {
        var idx = appController.projector_screen_index;
        var screens = Qt.application.screens;
        if (idx >= 0 && idx < screens.length)
            return idx;
        return 0;
    }

    flags: Qt.FramelessWindowHint
    color: appController.projection_background_color
    screen: Qt.application.screens[screenIndex]
    visibility: appController.projection_visible ? Window.FullScreen : Window.Hidden

    Item {
        id: textBlock
        anchors.centerIn: parent
        width: Math.max(0, parent.width - (appController.projection_margin_horizontal * 2))
        height: Math.max(0, parent.height - (appController.projection_margin_vertical * 2))

        property bool lineVisible: true

        function hAlign(value) {
            switch (value) {
            case "left":
                return Text.AlignLeft;
            case "right":
                return Text.AlignRight;
            default:
                return Text.AlignHCenter;
            }
        }

        function vAlign(value) {
            switch (value) {
            case "top":
                return Text.AlignTop;
            case "bottom":
                return Text.AlignBottom;
            default:
                return Text.AlignVCenter;
            }
        }

        function displayText(value) {
            return String(value).toUpperCase();
        }

        Connections {
            target: appController

            function onLyricTextChanged() {
                if (!appController.clear_screen && appController.lyric_text.length > 0) {
                    textBlock.lineVisible = false;
                    fadeTimer.restart();
                }
            }

            function onClearScreenChanged() {
                if (appController.clear_screen) {
                    textBlock.lineVisible = false;
                } else {
                    textBlock.lineVisible = true;
                }
            }
        }

        Timer {
            id: fadeTimer
            interval: 1
            repeat: false
            onTriggered: textBlock.lineVisible = true
        }

        Text {
            id: shadowText
            anchors.fill: parent
            anchors.leftMargin: appController.projection_shadow_enabled ? appController.projection_shadow_offset_x : 0
            anchors.topMargin: appController.projection_shadow_enabled ? appController.projection_shadow_offset_y : 0
            text: appController.clear_screen ? "" : textBlock.displayText(appController.lyric_text)
            color: appController.projection_shadow_color
            opacity: appController.clear_screen
                     ? 0
                     : (textBlock.lineVisible ? 1 : 0)
            visible: appController.projection_shadow_enabled
            wrapMode: Text.WordWrap
            horizontalAlignment: textBlock.hAlign(appController.projection_horizontal_alignment)
            verticalAlignment: textBlock.vAlign(appController.projection_vertical_alignment)
            font.family: appController.projection_font_family
            font.pixelSize: appController.projection_font_size
            font.weight: appController.projection_font_weight
            font.letterSpacing: appController.projection_letter_spacing
            lineHeightMode: Text.ProportionalHeight
            lineHeight: appController.projection_line_height_multiplier
            style: Text.Outline
            styleColor: appController.projection_shadow_color

            Behavior on opacity {
                enabled: appController.projection_fade_animation_enabled
                NumberAnimation { duration: appController.projection_fade_duration_ms }
            }
        }

        Text {
            id: lyricText
            anchors.fill: parent
            text: appController.clear_screen ? "" : textBlock.displayText(appController.lyric_text)
            color: appController.projection_font_color
            opacity: appController.clear_screen
                     ? 0
                     : (textBlock.lineVisible ? 1 : 0)
            wrapMode: Text.WordWrap
            horizontalAlignment: textBlock.hAlign(appController.projection_horizontal_alignment)
            verticalAlignment: textBlock.vAlign(appController.projection_vertical_alignment)
            font.family: appController.projection_font_family
            font.pixelSize: appController.projection_font_size
            font.weight: appController.projection_font_weight
            font.letterSpacing: appController.projection_letter_spacing
            lineHeightMode: Text.ProportionalHeight
            lineHeight: appController.projection_line_height_multiplier

            Behavior on opacity {
                enabled: appController.projection_fade_animation_enabled
                NumberAnimation { duration: appController.projection_fade_duration_ms }
            }
        }
    }
}
