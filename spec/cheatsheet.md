Manifest renders UI you write as MX text, and reads it back to you as the same text.
One node per line, two-space indent to nest. The first line names the surface: "@<id> [lifecycle]".

NODES (positional args; quote labels with spaces; bare words after a label must be valid flags)
  h1|h2|h3 Text             text Text [tone]          badge Text [tone]
  row [end|center|between]  col   card ["Title"]      grid <cols>   tabs > tab [*]Label (children = tab content)
  metric Label Value [Delta] [tone]           e.g. metric Churn 2.1% +0.4pt danger   (+ is green, - is red unless you say otherwise; the tone colours the delta)
  bar|line|area|pie ["Caption"] Key:Num ...   e.g. bar "Revenue" Apr:38 May:42 *Jun:51   (* highlights)
  table Col Col:badge ...  then one row per child line:  #1041 | Ana | $84 | Paid   (first cell = row key)
  flow ["Caption"]  then chain lines:  Agent >writes MX >draws Renderer:success   (Name:tone colours a node; trailing dashed|below)
  button Label [primary|danger|...] [disabled]     link Text <url>     image <src> "alt"
  field Label [email|date|number|search|password] [="value"] [required]
  select Label Opt *Opt Opt [multi]      check Label on|off      toggle Label on|off      slider Label <value> [min..max]
TONES: primary success danger warning info muted (the CSS-framework colour words).

GEOMETRY (any node): WxH makes a node a space whose children are all positioned with @x,y; a positioned node with no
  children gets its default look there (button Reset @400,280); with children it shows them instead (button "Export" @520,280 100x32
  over a rect and a text). Shapes: rect @x,y WxH | circle @cx,cy r:N | line @x1,y1 @x2,y2 | path "M 0 0 L 10 10 Z" | text @x,y "Str" | image @x,y WxH src "alt"
  Style words: fill: stroke: width: radius: opacity: size: weight: anchor:middle rotate: scale: dashed. Colours: tones, tone-70, fg fg2 bg card line white, #hex, none.
  A quoted string after a shape's geometry is its label (and id). region/item/figure "Label" are roles with no look of their own; decoration is invisible to readers.
  series name k:v ... is a buffer; =name binds a chart, =name.last a metric or text; each =name row repeats its children per element (=key =value); anim:300ms tweens.

IDS are derived, never written: first h1 -> title; labelled nodes -> first word of the label, lowercased
  ("Export CSV" -> export); other nodes -> their type (bar, table, row, h2). Duplicates inside a titled card or tab are
  scoped: pro.monthly. Otherwise numbered: row2. Perception prints #id wherever the id is not derivable. Add #id to choose one.

LIFECYCLE (on the @ line): ephemeral | ttl:10s | until:action | until:<buttonId> | seen:10s | residue:none|line|archive
  e.g. "@confirm until:action ttl:60s residue:line" dissolves when any button is pressed or after 60s, leaving one line.

PATCH (manifest.patch): first line "~<rev>" where rev = current rev + 1 (observe tells you the rev); then:
  *<addr> <args>      set: *revenue $315k +16%   *bar.Sep 80   *export enabled   *tabs General   *email ="a@b.c"
                           *class +Hatha   *table.#1044.Status Paid   *perception>agent solid   *note "New text" success
  +<addr> <node line> insert after addr; "+<addr>/ ..." appends inside it; "+table/ a | b | c" appends a row; "+sales Jul:47" a datum
  -<addr> remove (node, row or datum).  Geometry patches: *marker @300,180   *won 340x48 fill:warning
  Every address must resolve or the whole patch is rejected. A stale rev is rejected: observe, then retry.

OBSERVE (manifest.observe): level 0 = one line; 1 = the tree with live values and !diagnostics (inside a space, labelled nodes only);
  2 = plus @x,y,w,h boxes; 3 = every shape as written, with boxes. Label the arrows that matter, or write a flow: level 1 omits unlabelled shapes.
  since=<rev> returns only the changes as a patch. focus=<id> returns one subtree. wait=<seconds> blocks until a user event arrives.
  A new surface is rev 0; each patch and each user input adds one. manifest.look returns pixels of one node.
EVENTS arrive at the top of every tool result:  @form action book rev=5 {"email":"..."}   @form input email ="x" rev=6
  @toast dissolved reason=ttl:10s rev=0.  You never need to poll: call a tool and the events since the last call are there.
The full specification is the resource manifest://spec.
