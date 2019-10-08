# Insights
A utility written in Rust for querying data from New Relic Insights.

## Specifying the Account to Use

You will need your New Relic Account ID and an Insights API Query API Key.

This will list all the event types in the account.

`insights --account_id=123456 --api_key=dafserwqlouihafds types`

as will this

`insights -i 123456 -k dafserwqlouihafds types`

To avoid having to include the `account_id` and `api_key`, create a config file named `~/.insights.yaml`.

## Example Config File

```
default: personal
accounts:
  personal:
    account_id: 123456
    api_key: dafserwqlouihafds
```

With the config file in place, you can use:

`insights --account=personal types`

or

`insights -a personal types`

or

`insights types`

The latter works only because a `default` account was specfied in the config file.

## Examples

Get a list of all event types: `insights types`

Get a list of all attributes for an event type: `insights attrs NrDailyUsage`

Get a list of all unique values for an attribute: `insights complete NrDailyUsage agentHostname`

Get a list of all unique values for an attribute that begin with the given prefix ('u' in this case): `insights complete NrDailyUsage agentHostname u`

Run a NRQL query: `insights run "select * from NrDailyUsage since 1 week ago"`

## Limitations

* Event Types are case sensitive. If the wrong case is used, no results will be returned.
* Error handling is poor. The program panics more often than not when an error is encountered.
* No prebuilt binaries are available.
* No support for shell completion.
