# Null carries its own harness

Null first started whichever Oh-my-pi was installed on the Mac (`Providers.md`, decision 3: "It does
not download or run an installer"). Oh-my-pi publishes several releases a day, so any `omp update`
could change what Null runs, and a newcomer had to install the harness in a terminal before Null
could answer. The owner reversed that on 2026-10-09: "Null should use OMP as it's harness and
should be installed in there automatically", and "Is there a way that we can have null be
unaffected by the update of omp?"

Null therefore carries Oh-my-pi inside the app, at one version named in `Mini/Engine.toml`, and
starts that one. The version moves only after the harness check has passed on the newer one. The
harness still uses Oh-my-pi's ordinary folder, so sign-ins, skills, instructions and MCP servers
are the user's own and no credential rests in Null. A technical person can point Null at their own
Oh-my-pi with `/harness`, and anyone can move to the newest checked version with `/update`.

The cost is accepted: the app grows from 12 MB to about 200 MB, and Null takes on moving the
harness forward.

Rejected: starting the Oh-my-pi found on the Mac, as before (nothing works until it is installed,
and every update of it changes Null); always using the newest version (several releases a day).
A folder of Null's own for the harness is kept only as the fallback, if versions far apart cannot
share one folder.
