# 0482. A new pose from a client stands the supports anew

- **Status:** Proposed
- **Date:** 2026-11-26

## Context

By ADR 0439 a turned model keeps its support contacts in its own space. Its grown trees are
dropped and grown again from those contacts where it now stands. This suits a tilt of a few
degrees by hand in the window. `encrust-mcp` makes the same carry when `set_pose` changes the pose
of a prepared model. It hands the plate a `Rotate` or an `Orient` edit (ADR 0466), and those
edits carry the contacts.

On a user's diorama, the autopilot laid the model nearly flat. A client then stood it upright
at `[35, -30, -10]`. Most of the old contacts now sat on faces looking sideways or up. No tree
grew from them (`FacesUp`, `HeadBlocked`, `NoGround`), or a tree stood on a face meant to be
seen. The faces that now looked down had no contact, and the check found 69 islands. The client
then asked for auto supports. A `Supports` edit starts from the contacts that stand and takes
them as seeds, so the new supports were added to what the old pose left. No edit could take
every support away. The flags path builds supports from nothing for the pose it stands, so it
never had this defect.

## Decision

- **`Edit::Supports` takes `anew`** (default `false`). With it, the document and the window
  take every support of the object away first: points and frozen trees. The painted blocked and
  shown regions stay, then the run goes on as before.
- **When a revision changes the pose of a model, `encrust-mcp` follows the turn with a
  `Supports` edit that has `anew`, wherever the flags path would stand supports.** The edit
  takes its settings from the recipe's supports: their profile and `cavity`. A prepared model
  with none takes the autopilot's: `auto`, with the cavity held where the model is hollowed. A
  model with neither gets the edit only where the previous check reports supports under it
  (`models[0].supports.contacts`), as the window's hand placed them; it takes the plate's active
  group. A plate whose trees are grown again on opening reports no supports, so the recipe
  decides first. A model whose supports come from a recipe's own steps, or from points given by
  hand, is left to the flags path, which builds them from nothing.
- **`set_pose` clears the support points, shown points and merges given by hand**, as it
  clears drain points: they were plate mm on the old pose. Asking for those points alone is
  lifted with them, so the automatic run stands.

## Consequences

After a new pose, the supports are those the engine stands for that pose from nothing. They
are the supports the flags path builds, so no turn leaves fewer or worse ones. The client no
longer needs `set_supports` after `set_pose`. Supports a user grew or edited by hand in the
window are lost on a client's turn. They stood on the old pose, and the flags path loses them
too. A tilt by hand in the window still carries its contacts as ADR 0439 decided. Reopen if a
client's small turns should carry the contacts as the window's do.

## Alternatives considered

### Replay the recipe by its flags on every new pose

It needs no new field and builds the supports from nothing. But every other edit the user made
in the window since the job began is lost, and the window is shown a whole new plate rather
than handed the edits.

### Keep the contacts that still face down after the turn

It keeps more of what stood. But a contact chosen for the old pose is not where the planner
would put one for the new pose, so the supports get worse with the turn.

### The option that won, and what it costs

A field on an edit that crosses the window link, and one more decision in the server about when
to send it. For supports placed by hand in the window, the decision reads the previous check's
report. A plate that reports none of them keeps them carried as ADR 0439 carries them.
