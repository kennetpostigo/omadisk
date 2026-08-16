import QtQuick
import qs.Commons
import "Format.js" as Format

Item {
  id: root

  property var overlay: null
  property color foreground: Color.menu.text
  property color selectedBackground: Color.menu.selectedBackground
  property int rowHeight: Math.max(Style.space(28), Style.font.body + Style.spacing.rowPaddingX)

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

    delegate: Rectangle {
      required property var modelData
      required property int index
      width: list.width
      height: root.rowHeight
      radius: Style.cornerRadius
      color: {
        if (!overlay) return "transparent"
        if (overlay.hoverPath === modelData.path || overlay.selectedIndex === index)
          return root.selectedBackground
        return "transparent"
      }

      Row {
        anchors.fill: parent
        anchors.leftMargin: Style.space(8)
        anchors.rightMargin: Style.space(8)
        spacing: Style.space(8)

        Text {
          anchors.verticalCenter: parent.verticalCenter
          width: parent.width - sizeLabel.width - Style.space(16)
          text: (modelData.error === "permission" ? "🔒 " : "") + (modelData.name || "")
          color: root.foreground
          font.family: Style.font.menuFamily
          font.pixelSize: Style.font.body
          elide: Text.ElideRight
        }

        Text {
          id: sizeLabel
          anchors.verticalCenter: parent.verticalCenter
          text: Format.bytes(modelData.bytes)
          color: root.foreground
          opacity: 0.58
          font.family: Style.font.menuFamily
          font.pixelSize: Style.font.caption
        }
      }

      MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: modelData.kind === "dir" ? Qt.PointingHandCursor : Qt.ArrowCursor
        onEntered: {
          if (!overlay) return
          overlay.hoverPath = modelData.path
          overlay.selectedIndex = index
        }
        onClicked: {
          if (!overlay) return
          overlay.selectedIndex = index
          overlay.hoverPath = modelData.path
          if (modelData.kind === "dir") overlay.drill(modelData.path)
        }
      }
    }

    footer: Item {
      visible: overlay && overlay.currentView && overlay.currentView.listTruncated > 0
      width: list.width
      height: visible ? root.rowHeight : 0
      Text {
        anchors.centerIn: parent
        color: root.foreground
        opacity: 0.58
        font.family: Style.font.menuFamily
        font.pixelSize: Style.font.caption
        text: overlay && overlay.currentView
          ? "and " + Format.count(overlay.currentView.listTruncated) + " smaller items"
          : ""
      }
    }
  }
}
