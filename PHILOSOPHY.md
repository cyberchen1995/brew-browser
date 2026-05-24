PHILOSOPHY.md

brew-browser Philosophy

brew-browser exists because modern developer tools are increasingly asking users to trade transparency for convenience.

We reject that trade.

This application is intentionally designed to remain:

* local-first
* inspectable
* deterministic
* respectful of user agency
* aligned with the Unix and Homebrew ecosystem

brew-browser is not trying to replace Homebrew.

It is a visibility and control layer built on top of Homebrew.

The terminal remains the source of truth.

⸻

Principles

1. Homebrew Comes First

Homebrew already solves package management extremely well.

brew-browser does not attempt to abstract Homebrew into a proprietary ecosystem, alternate package manager, or closed platform.

Everything ultimately maps back to standard Homebrew behavior:

* formulae
* casks
* services
* taps
* Brewfiles
* launchctl-managed processes

The goal is understanding and visibility — not enclosure.

⸻

2. Local-First by Default

brew-browser should function without requiring:

* accounts
* cloud sync
* telemetry
* subscriptions
* remote execution
* behavioral tracking

Your machine is your machine.

Most functionality operates entirely locally using:

* the Homebrew CLI
* local metadata
* bundled indexes
* local filesystem inspection

Remote network requests are minimized, explicit, and controllable.

⸻

3. No Telemetry

brew-browser does not collect:

* analytics
* usage metrics
* click tracking
* package installation behavior
* device fingerprints
* behavioral profiles

There are no hidden analytics SDKs.

There are no background event streams.

There is no user surveillance disguised as “product improvement.”

If something requires network access, it should be understandable and inspectable.

⸻

4. Transparency Over Magic

Modern software increasingly hides behavior behind opaque systems:

* silent network requests
* AI-generated behavior
* hidden recommendation systems
* undisclosed ranking algorithms
* dark-pattern UX

brew-browser intentionally avoids this model.

If the application:

* categorizes packages
* surfaces trends
* performs discovery
* displays metadata

…those systems should be explainable.

Semantic package categorization is generated offline using small language models, reviewed and normalized, then bundled into application releases as static metadata.

No runtime AI dependency exists inside the app.

⸻

5. Deterministic UX

Developer tools should behave predictably.

brew-browser favors:

* explicit actions
* stable interfaces
* understandable state
* reversible operations
* inspectable outputs

Over:

* probabilistic behavior
* engagement loops
* recommendation manipulation
* “smart” automation that obscures system state

The application should help users understand their system — not hide it.

⸻

6. Security Is a Feature

Convenience is not an excuse for unsafe architecture.

brew-browser intentionally avoids:

* arbitrary shell interpolation from the frontend
* embedded remote execution layers
* unnecessary privilege escalation
* opaque plugin systems

The application uses typed Rust command boundaries rather than unrestricted shell bridging.

Security decisions are made conservatively, especially around:

* filesystem access
* remote requests
* icon fetching
* service management
* authentication flows

⸻

7. Respect the Ecosystem

brew-browser exists because of the work of:

* Homebrew maintainers
* formula maintainers
* open-source developers
* Unix tooling communities

This project is not an attempt to “own” that ecosystem.

The intent is to make the ecosystem:

* easier to understand
* easier to navigate
* easier to manage
* easier to trust

Without compromising the values that made it valuable in the first place.

⸻

What brew-browser Is Not

brew-browser is not:

* an App Store
* a social platform
* a telemetry business
* an engagement engine
* a recommendation algorithm company
* a SaaS product disguised as desktop software
* a closed wrapper around open infrastructure

It is a local utility application.

That distinction matters.

⸻

Design Philosophy

The interface intentionally favors:

* information density
* clarity
* restrained visuals
* operational visibility
* low cognitive overhead

Over:

* decorative complexity
* dashboard theater
* artificial gamification
* attention extraction

The design goal is calm competence.

⸻

AI Usage

AI is used as infrastructure, not spectacle.

Examples:

* semantic categorization pipelines
* metadata enrichment
* offline indexing workflows

AI is not used for:

* user surveillance
* behavioral ranking
* opaque runtime decisions
* engagement optimization

The best AI systems often disappear into the product entirely.

⸻

Long-Term Direction

brew-browser aims to become a trustworthy local systems console for the Homebrew ecosystem:

* package visibility
* service management
* storage awareness
* reproducible environments
* snapshotting
* discovery
* operational diagnostics

Without compromising:

* transparency
* local ownership
* determinism
* user trust

⸻

License

MIT.

Because users should own the software running on their machines.
