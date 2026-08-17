import QtQuick
import qs.Commons
import "Format.js" as Format

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text
  property color selectedBackground: Style.hoverFillFor(foreground, Color.accent)
  property int rowHeight: Math.max(Style.space(22), Style.font.body + Style.space(6))

  function pageRows() {
    return Math.max(1, Math.floor(list.height / rowHeight))
  }

  function positionSelected() {
    if (!overlay) return
    if (overlay.selectedIndex >= 0)
      list.positionViewAtIndex(overlay.selectedIndex, ListView.Contain)
  }

  ListView {
    id: list
    anchors.fill: parent
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: overlay ? overlay.listRows : []
    spacing: 0

    delegate: Item {
      required property var modelData
      required property int index
      width: list.width
      height: root.rowHeight

      readonly property bool active: overlay && (overlay.hoverPath === modelData.path || overlay.selectedIndex === index)

      Rectangle {
        width: 2
        height: parent.height - Style.space(6)
        radius: 1
        anchors.verticalCenter: parent.verticalCenter
        color: parent.active ? Color.accent : "transparent"
      }

      Rectangle {
        anchors.fill: parent
        color: parent.active ? root.selectedBackground : "transparent"
        radius: Style.cornerRadius
      }

      Row {
        anchors.fill: parent
        anchors.leftMargin: Style.space(10)
        anchors.rightMargin: Style.space(4)
        spacing: Style.space(8)

        Text {
          anchors.verticalCenter: parent.verticalCenter
          width: parent.width - sizeLabel.width - Style.space(12)
          text: modelData.name || ""
          color: root.foreground
          opacity: active ? 1 : 0.72
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          elide: Text.ElideRight
        }

        Text {
          id: sizeLabel
          anchors.verticalCenter: parent.verticalCenter
          text: Format.bytes(modelData.bytes)
          color: root.foreground
          opacity: 0.45
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }
      }

      MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        preventStealing: true
        cursorShape: modelData.kind === "dir" ? Qt.PointingHandCursor : Qt.ArrowCursor
        onEntered: if (overlay) overlay.syncSelection(modelData.path)
        onPressed: {
          if (!overlay) return
          overlay.activatePath(modelData.path)
        }
      }
    }

    footer: Item {
      visible: overlay && overlay.currentView && overlay.currentView.listTruncated > 0
      width: list.width
      height: visible ? root.rowHeight : 0
      Text {
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.leftMargin: Style.space(10)
        color: root.foreground
        opacity: 0.4
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        text: overlay && overlay.currentView
          ? "+" + Format.count(overlay.currentView.listTruncated)
          : ""
      }
    }
  }
}
