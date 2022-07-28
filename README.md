# nu_plugin_net
A nushell plugin to list system network interfaces

A simple and straightforward plugin. All of the heavy lifting is done by pnet. This package just translates the datatypes into a nu-accepatble format.

Format may be subject to change.

# Examples

```
~> net
╭───┬──────────┬─────────────┬───────────────────┬────────────────┬───────────────────╮
│ # │   name   │ description │        mac        │      ips       │       flags       │
├───┼──────────┼─────────────┼───────────────────┼────────────────┼───────────────────┤
│ 1 │ lo       │             │ 00:00:00:00:00:00 │ [table 2 rows] │ {record 5 fields} │
│ 2 │ enp2s0f0 │             │ 8c:8c:aa:1f:a5:2a │ [table 2 rows] │ {record 5 fields} │
│ 3 │ wlp3s0   │             │ c8:e2:65:c3:09:42 │ [table 2 rows] │ {record 5 fields} │
╰───┴──────────┴─────────────┴───────────────────┴────────────────┴───────────────────╯
```

```
~> net | flatten flags                                                                                                                                                                                                                                                                            07/28/2022 04:36:27 PM
╭───┬──────────┬─────────────┬───────────────────┬────────────────┬───────┬──────────────┬─────────────┬───────────────────┬──────────────╮
│ # │   name   │ description │        mac        │      ips       │ is_up │ is_broadcast │ is_loopback │ is_point_to_point │ is_multicast │
├───┼──────────┼─────────────┼───────────────────┼────────────────┼───────┼──────────────┼─────────────┼───────────────────┼──────────────┤
│ 1 │ lo       │             │ 00:00:00:00:00:00 │ [table 2 rows] │ true  │ false        │ true        │ false             │ false        │
│ 2 │ enp2s0f0 │             │ 8c:8c:aa:1f:a5:2a │ [table 2 rows] │ true  │ true         │ false       │ false             │ true         │
│ 3 │ wlp3s0   │             │ c8:e2:65:c3:09:42 │ [table 2 rows] │ true  │ true         │ false       │ false             │ true         │
╰───┴──────────┴─────────────┴───────────────────┴────────────────┴───────┴──────────────┴─────────────┴───────────────────┴──────────────╯
```

```
~> net | select ips | flatten | flatten                                                                                                                                                                                                                                                           07/28/2022 04:37:37 PM
╭───┬──────┬──────────────────────────────┬────────╮
│ # │ type │             addr             │ prefix │
├───┼──────┼──────────────────────────────┼────────┤
│ 0 │ v4   │ 127.0.0.1/8                  │      8 │
│ 1 │ v6   │ ::1/128                      │    128 │
│ 2 │ v4   │ 192.168.1.232/24             │     24 │
│ 3 │ v6   │ fe80::8e8c:aaff:fe1f:a52a/64 │     64 │
│ 4 │ v4   │ 192.168.4.189/24             │     24 │
│ 5 │ v6   │ fe80::cae2:65ff:fec3:942/64  │     64 │
╰───┴──────┴──────────────────────────────┴────────╯
```
