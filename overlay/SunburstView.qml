import QtQuick
import qs.Commons
import "OverlayModel.js" as Model

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text
  property real fadeOpacity: 1

  readonly property real sunburstSize: Math.min(width, height)
  readonly property real cx: width / 2
  readonly property real cy: height / 2
  readonly property real hubR: 0.34 * sunburstSize / 2
  readonly property real ringW: 0.18 * sunburstSize / 2
  readonly property real ringPad: Style.space(3)
  readonly property real outerR: hubR + 3 * (ringW + ringPad)

  opacity: fadeOpacity

  Behavior on fadeOpacity {
    NumberAnimation { duration: 140; easing.type: Easing.OutCubic }
  }

  Repeater {
    model: overlay ? overlay.sliceModel : null
    SliceArc {
      required property int index
      anchors.fill: parent
      overlay: root.overlay
      sliceIndex: index
      cx: root.cx
      cy: root.cy
    }
  }

  HubLabel {
    width: root.hubR * 2
    height: root.hubR * 2
    anchors.centerIn: parent
    overlay: root.overlay
    foreground: root.foreground
  }

  EmptyState {
    anchors.fill: parent
    overlay: root.overlay
    foreground: root.foreground
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    preventStealing: true
    cursorShape: Qt.ArrowCursor
    onPositionChanged: function(mouse) {
      if (!overlay) return
      var hit = Model.hitTest(mouse.x, mouse.y, root.cx, root.cy, overlay.sliceModel, root.hubR, root.outerR)
      if (hit.kind === "slice") {
        overlay.syncSelection(hit.slice.path)
        cursorShape = hit.slice.kind === "dir" ? Qt.PointingHandCursor : Qt.ArrowCursor
      } else if (hit.kind === "hub") {
        overlay.syncSelection(overlay.focusPath)
        cursorShape = Qt.PointingHandCursor
      }
    }
    onPressed: function(mouse) {
      if (!overlay) return
      var hit = Model.hitTest(mouse.x, mouse.y, root.cx, root.cy, overlay.sliceModel, root.hubR, root.outerR)
      if (hit.kind === "hub") {
        overlay.goUp()
        return
      }
      if (hit.kind === "slice")
        overlay.activatePath(hit.slice.path)
    }
  }
}
