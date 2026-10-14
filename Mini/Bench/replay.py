"""A task's example end states, made by doing its actions on a fresh world.

Each task carries a right end state and one or more wrong ones, as the actions
a page would have sent and the answer that would have been given. Doing them
through `World.act`, as the server does, gives end states a checker can be
tried on: it must pass the right one and fail every wrong one. This is how each
checker is tested before any run is measured on it.
"""

import datetime

from world import World

# A Monday, so that the examples' days of the week are the same on any day they are tried.
BASE = datetime.date(2026, 10, 12)


def _resolve(value, base):
    """`@day:2026-10-24`, in an example, is that date as the calendar counts days."""
    if isinstance(value, str) and value.startswith("@day:"):
        return (datetime.date.fromisoformat(value[5:]) - base).days
    if isinstance(value, dict):
        return {k: _resolve(v, base) for k, v in value.items()}
    if isinstance(value, list):
        return [_resolve(v, base) for v in value]
    return value


def end_state(example, base=BASE):
    """The world after an example's actions, what each action was told, and its answer."""
    world = World(base=base)
    told = []
    for app, action, args in example["do"]:
        status, said = world.act(app, action, _resolve(args, base))
        told.append((f"{app}.{action}", status, said))
    return world.end(), told, example["reply"]
