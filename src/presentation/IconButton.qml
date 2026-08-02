import QtQuick
import QtQuick.Controls

Button {
    id: iconBtn
    property color idleColor: "#A0A4B8"
    property color hoverColor: "#FFFFFF"
    property string tip: ""
    property int iconPixelSize: 15
    hoverEnabled: true
    flat: true
    padding: 8
    leftPadding: 10
    rightPadding: 10
    ToolTip.visible: iconBtn.tip.length > 0 && iconBtn.hovered
    ToolTip.text: iconBtn.tip
    ToolTip.delay: 500

    background: Rectangle {
        radius: 8
        color: iconBtn.down ? "#242935" : (iconBtn.hovered ? "#1C1F29" : "transparent")
    }
    contentItem: Text {
        text: iconBtn.text
        color: iconBtn.enabled ? (iconBtn.hovered ? iconBtn.hoverColor : iconBtn.idleColor) : "#4A4E5C"
        font.family: "Inter"
        font.pixelSize: iconBtn.iconPixelSize
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }
}
