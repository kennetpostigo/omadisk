import QtQuick
import QtQuick.Shapes
import qs.Commons
import "OverlayModel.js" as Model

Item {
  id: root

  property var overlay: null
  property color foreground: Color.menu.text
  property real fadeOpacity: 1

  readonly property real sunburstSize: Math.min(width, height)
  readonly property real cx: width / 2
  readonly property real cy: height / 2
  readonly property real hubR: 0.28 * sunburstSize / 2
  readonly property real ringW: 0.22 * sunburstSize / 2
  readonly property real ringPad: Style.space(2)
  readonly property real outerR: hubR + 3 * (ringW + ringPad)

  opacity: fadeOpacity

  Behavior on fadeOpacity {
    NumberAnimation { duration: 150; easing.type: Easing.OutCubic }
  }

  Shape {
    anchors.fill: parent
    preferredRendererType: Shape.CurveRenderer
    ShapePath {
      strokeWidth: 1
      strokeColor: Util.alpha(root.foreground, 0.18)
      fillColor: "transparent"
      PathAngleArc {
        centerX: root.cx
        centerY: root.cy
        radiusX: root.hubR
        radiusY: root.hubR
        startAngle: 0
        sweepAngle: 360
      }
    }
  }

  Repeater {
    model: overlay ? overlay.sliceModel : null
    SliceArc {
      anchors.fill: parent
      overlay: root.overlay
      slice: ({
        path: path,
        name: name,
        kind: kind,
        bytes: bytes,
        ring: ring,
        startDeg: startDeg,
        sweepDeg: sweepDeg,
        innerR: innerR,
        outerR: outerR,
        color: color,
        drillable: drillable
      })
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
    propagateComposedEvents: false
    onPositionChanged: function(mouse) {
      if (!overlay) return
      var hit = Model.hitTest(mouse.x, mouse.y, root.cx, root.cy, overlay.sliceModel, root.hubR, root.outerR)
      if (hit.kind === "slice") overlay.hoverPath = hit.slice.path
      else if (hit.kind === "hub") overlay.hoverPath = overlay.focusPath
      else overlay.hoverPath = ""
    }
    onClicked: function(mouse) {
      if (!overlay) return
      var hit = Model.hitTest(mouse.x, mouse.y, root.cx, root.cy, overlay.sliceModel, root.hubR, root.outerR)
      if (hit.kind === "hub") {
        overlay.goUp()
        return
      }
      if (hit.kind === "slice") {
        overlay.drill(hit.slice.path)
      }
    }
    onExited: if (overlay) overlay.hoverPath = ""
  }
}
