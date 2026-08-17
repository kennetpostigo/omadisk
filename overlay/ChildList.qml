import QtQuick
import qs.Commons
import "Format.js" as Format
import "OverlayModel.js" as Model

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text
  property color selectedBackground: Util.alpha(Color.accent, 0.22)
  property int rowHeight: Math.max(Style.space(20), Style.font.body + Style.space(4))

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
    spacing: 1

    delegate: Item {
      required property var modelData
      required property int index
      width: list.width
      height: root.rowHeight

      readonly property bool active: {
        if (!overlay || !modelData) return false
        void overlay.hoverTick
        return overlay.hoverPath === modelData.path
      }
      readonly property string swatch: overlay ? (Model.fillForPath(overlay.slices, modelData.path) || "") : ""

      Rectangle {
        anchors.fill: parent
        radius: Style.cornerRadius
        color: parent.active ? root.selectedBackground : "transparent"
      }

      Rectangle {
        width: Style.space(6)
        height: Style.space(6)
        radius: width / 2
        anchors.left: parent.left
        anchors.leftMargin: Style.space(6)
        anchors.verticalCenter: parent.verticalCenter
        color: parent.swatch || Util.alpha(root.foreground, 0.22)
        opacity: parent.active ? 1 : 0.8
      }

      Row {
        anchors.fill: parent
        anchors.leftMargin: Style.space(16)
        anchors.rightMargin: Style.space(4)
        spacing: Style.space(6)

        Text {
          anchors.verticalCenter: parent.verticalCenter
          width: parent.width - sizeLabel.width - Style.space(12)
          text: modelData.name || ""
          color: root.foreground
          opacity: active ? 1 : 0.78
          font.family: Style.font.family
          font.pixelSize: Style.font.body
          elide: Text.ElideRight
        }

        Text {
          id: sizeLabel
          anchors.verticalCenter: parent.verticalCenter
          text: Format.bytes(modelData.bytes)
          color: root.foreground
          opacity: active ? 0.85 : 0.48
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
        onPositionChanged: if (overlay) overlay.syncSelection(modelData.path)
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
        anchors.leftMargin: Style.space(16)
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
