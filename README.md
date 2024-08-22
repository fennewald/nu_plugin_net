# TODO: shout out nu_plugin_dns


# nu_plugin_net
A nushell plugin to list system network interfaces

A simple and straightforward plugin. All of the heavy lifting is done by pnet. This package just translates the datatypes into a nu-accepatble format.

Format may be subject to change.

# Version 2.0 Is in the Works

Version 2 of this plugin is actively being prepared. Some imporant objectives:
* Automate the nushell version update process
* Setup a website, more clear installation instructions
* Add support for additional commands
  - `ping`
  - Some form of tcp SYN port-scanning
  - Speed testing (50-50 about this one, please comment thoughts)
  - ARP ping, listening
  - Interface for viewing routing tables
  - traceroute (This might be too difficult, we'll see)

If you've got thoughts about the direction we should take, or are interested in helping out, please get in touch! Feel free to file an issue, or just send me an email directly.

# Examples

```
~> net
╭───┬──────┬─────────────┬──────────┬───────────────────┬──────────────────────────────────────────────────┬────────────────────────────────╮
│ # │ name │ description │ if_index │        mac        │                       ips                        │             flags              │
├───┼──────┼─────────────┼──────────┼───────────────────┼──────────────────────────────────────────────────┼────────────────────────────────┤
│ 0 │ lo   │             │        1 │ 00:00:00:00:00:00 │ ╭───┬───────────┬──────┬────────╮                │ ╭────────────────────┬───────╮ │
│   │      │             │          │                   │ │ # │   addr    │ type │ prefix │                │ │ is_up              │ true  │ │
│   │      │             │          │                   │ ├───┼───────────┼──────┼────────┤                │ │ is_broadcast       │ false │ │
│   │      │             │          │                   │ │ 0 │ 127.0.0.1 │ v4   │      8 │                │ │ is_loopback        │ true  │ │
│   │      │             │          │                   │ │ 1 │ ::1       │ v6   │    128 │                │ │ is_point_to_point  │ false │ │
│   │      │             │          │                   │ ╰───┴───────────┴──────┴────────╯                │ │ is_multicast       │ false │ │
│   │      │             │          │                   │                                                  │ ╰────────────────────┴───────╯ │
│ 1 │ ens5 │             │        2 │ 0e:ec:4c:2a:2e:43 │ ╭───┬──────────────────────────┬──────┬────────╮ │ ╭────────────────────┬───────╮ │
│   │      │             │          │                   │ │ # │           addr           │ type │ prefix │ │ │ is_up              │ true  │ │
│   │      │             │          │                   │ ├───┼──────────────────────────┼──────┼────────┤ │ │ is_broadcast       │ true  │ │
│   │      │             │          │                   │ │ 0 │ 172.23.65.24             │ v4   │     24 │ │ │ is_loopback        │ false │ │
│   │      │             │          │                   │ │ 1 │ fe80::cec:4cff:fe2a:2e43 │ v6   │     64 │ │ │ is_point_to_point  │ false │ │
│   │      │             │          │                   │ ╰───┴──────────────────────────┴──────┴────────╯ │ │ is_multicast       │ true  │ │
│   │      │             │          │                   │                                                  │ ╰────────────────────┴───────╯ │
╰───┴──────┴─────────────┴──────────┴───────────────────┴──────────────────────────────────────────────────┴────────────────────────────────╯
```

```
~> net | flatten flags
╭───┬──────┬─────────────┬──────────┬───────────────────┬──────────────────────────────────────────────────┬───────┬──────────────┬─────────────┬───────────────────┬──────────────╮
│ # │ name │ description │ if_index │        mac        │                       ips                        │ is_up │ is_broadcast │ is_loopback │ is_point_to_point │ is_multicast │
├───┼──────┼─────────────┼──────────┼───────────────────┼──────────────────────────────────────────────────┼───────┼──────────────┼─────────────┼───────────────────┼──────────────┤
│ 0 │ lo   │             │        1 │ 00:00:00:00:00:00 │ ╭───┬───────────┬──────┬────────╮                │ true  │ false        │ true        │ false             │ false        │
│   │      │             │          │                   │ │ # │   addr    │ type │ prefix │                │       │              │             │                   │              │
│   │      │             │          │                   │ ├───┼───────────┼──────┼────────┤                │       │              │             │                   │              │
│   │      │             │          │                   │ │ 0 │ 127.0.0.1 │ v4   │      8 │                │       │              │             │                   │              │
│   │      │             │          │                   │ │ 1 │ ::1       │ v6   │    128 │                │       │              │             │                   │              │
│   │      │             │          │                   │ ╰───┴───────────┴──────┴────────╯                │       │              │             │                   │              │
│ 1 │ ens5 │             │        2 │ 0e:ec:4c:2a:2e:43 │ ╭───┬──────────────────────────┬──────┬────────╮ │ true  │ true         │ false       │ false             │ true         │
│   │      │             │          │                   │ │ # │           addr           │ type │ prefix │ │       │              │             │                   │              │
│   │      │             │          │                   │ ├───┼──────────────────────────┼──────┼────────┤ │       │              │             │                   │              │
│   │      │             │          │                   │ │ 0 │ 172.23.65.24             │ v4   │     24 │ │       │              │             │                   │              │
│   │      │             │          │                   │ │ 1 │ fe80::cec:4cff:fe2a:2e43 │ v6   │     64 │ │       │              │             │                   │              │
│   │      │             │          │                   │ ╰───┴──────────────────────────┴──────┴────────╯ │       │              │             │                   │              │
╰───┴──────┴─────────────┴──────────┴───────────────────┴──────────────────────────────────────────────────┴───────┴──────────────┴─────────────┴───────────────────┴──────────────╯
```

# Installing

This plugin can either be installed from crates.io, or built from source.

To install using cargo, run:
```
cargo install nu_plugin_net
```

To build from source, use:
```
git clone https://github.com/fennewald/nu_plugin_net.git
cd nu_plugin_net
cargo install --path .
```

Both of these processes will place a binary in `~/.cargo/bin/nu_plugin_net`
To register the plugin for use, just run:
```
plugin add ~/.cargo/bin/nu_plugin_net
```

# Changelog

## Version 1.7.0

* Update to Nushell 0.97.1
* Reorganized the code, preparing for a full rewrite.

## Version 1.6.0

* Update for Nushell 0.96.0

## Version 1.5.0

* Update for Nushell 0.94.2

Maintainer note: Sorry for the inconsitencies. From now on, nushell version updates will include minor version bumps

## Version 1.4.1

* (@baerlkr) Update for Nushell 0.93

## Version 1.4.0

* (@oraoto) Update for Nushell 0.92

## Version 1.3.0

* (@FMotalleb) Bump dependency versions
* (@FMotalleb) Refactor: replaced structs with standard constructors

## Version 1.2.0

* Update for Nushell 0.84

## Version 1.1.0

* Use `if_index` instead of `index`, fixing the way table indexes are displayed
