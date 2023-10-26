# banking-trace-tool

Personal utility tool for scanning through Solana banking-trace data

```bash
Usage: banking-trace-tool --path <PATH> <COMMAND>

Commands:
  account-usage     Get account usage statistics for a given slot range
  slot-ranges       Get the ranges of slots for data in directory
  update-alt-store  Update Address-Lookup-Table store for tables used in a given slot-range
  help              Print this message or the help of the given subcommand(s)

Options:
  -p, --path <PATH>  The path to the banking trace event file directory
  -h, --help         Print help
```
