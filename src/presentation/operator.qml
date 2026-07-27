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
                text: "Carregando..."
            }
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

        Item {
            Layout.fillHeight: true
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 4

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
                text: appController.projection_visible ? "Ocultar" : "Projetar"
                onClicked: appController.toggle_projection()
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

    function formatTime(seconds) {
        if (isNaN(seconds) || seconds < 0)
            return "0:00";
        var total = Math.floor(seconds);
        var mins = Math.floor(total / 60);
        var secs = total % 60;
        return mins + ":" + (secs < 10 ? "0" + secs : secs);
    }
}
