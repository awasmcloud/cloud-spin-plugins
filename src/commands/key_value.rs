use crate::commands::links_output::{
    print_json, print_table, prompt_delete_resource, ListFormat, ResourceGroupBy, ResourceLinks,
    ResourceType,
};
use crate::commands::links_target::ResourceTarget;
use crate::commands::{create_cloud_client, disallow_empty, CommonArgs};
use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use cloud::CloudClientInterface;
use cloud_openapi::models::KeyValueStoreItem;
use spin_common::arg_parser::parse_kv;

#[derive(Parser, Debug)]
#[clap(about = "Manage Fermyon Cloud key value stores")]
pub enum KeyValueCommand {
    /// Create a new key value store
    Create(CreateCommand),
    /// Delete a key value store
    Delete(DeleteCommand),
    /// List key value stores
    List(ListCommand),
    /// Set a key value pair in a store
    Set(SetCommand),
    /// Rename a key value store. All existing links will automatically link to the store's new name.
    Rename(RenameCommand),
}

#[derive(Parser, Debug)]
pub struct CreateCommand {
    /// The name of the key value store
    pub name: String,

    #[clap(flatten)]
    common: CommonArgs,
}

#[derive(Parser, Debug)]
pub struct DeleteCommand {
    /// The name of the key value store
    pub name: String,

    /// Skips prompt to confirm deletion of the key value store
    #[clap(short = 'y', long = "yes", takes_value = false)]
    yes: bool,

    #[clap(flatten)]
    common: CommonArgs,
}

#[derive(Parser, Debug)]
pub struct ListCommand {
    /// Filter list by an app
    #[clap(short = 'a', long = "app")]
    app: Option<String>,
    /// Filter list by a key value store
    #[clap(short = 's', long = "store")]
    store: Option<String>,
    /// Grouping strategy of tabular list [default: app]
    #[clap(value_enum, short = 'g', long = "group-by")]
    group_by: Option<GroupBy>,
    /// Format of list
    #[clap(value_enum, long = "format", default_value = "table")]
    format: ListFormat,
    #[clap(flatten)]
    common: CommonArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
enum GroupBy {
    #[default]
    App,
    Store,
}

impl From<GroupBy> for ResourceGroupBy {
    fn from(group_by: GroupBy) -> Self {
        match group_by {
            GroupBy::App => Self::App,
            GroupBy::Store => Self::Resource(ResourceType::KeyValueStore),
        }
    }
}

#[derive(Parser, Debug)]
pub struct SetCommand {
    /// The name of the key value store
    #[clap(name = "STORE", short = 's', long = "store", value_parser = clap::builder::ValueParser::new(disallow_empty), required_unless_present_all = ["LABEL", "APP"], conflicts_with_all = &["LABEL", "APP"])]
    pub store: Option<String>,

    /// Label of the key value store to set pairs in
    #[clap(name = "LABEL", short = 'l', long = "label", value_parser = clap::builder::ValueParser::new(disallow_empty), requires = "APP", required_unless_present = "STORE")]
    pub label: Option<String>,

    /// App to which label relates
    #[clap(name = "APP", short = 'a', long = "app", value_parser = clap::builder::ValueParser::new(disallow_empty), requires = "LABEL", required_unless_present = "STORE")]
    pub app: Option<String>,

    /// A key/value pair (key=value) to set in the store. Any existing value will be overwritten.
    /// Can be used multiple times.
    #[clap(parse(try_from_str = parse_kv))]
    pub key_values: Vec<(String, String)>,

    #[clap(flatten)]
    common: CommonArgs,
}

#[derive(Parser, Debug)]
pub struct RenameCommand {
    /// Current name of key value store to rename
    name: String,

    /// New name for the key value store
    new_name: String,

    #[clap(flatten)]
    common: CommonArgs,
}

impl KeyValueCommand {
    pub async fn run(&self) -> Result<()> {
        match self {
            KeyValueCommand::Create(cmd) => {
                let client = create_cloud_client(cmd.common.deployment_env_id.as_deref()).await?;
                cmd.run(client).await
            }
            KeyValueCommand::Delete(cmd) => {
                let client = create_cloud_client(cmd.common.deployment_env_id.as_deref()).await?;
                cmd.run(client).await
            }
            KeyValueCommand::List(cmd) => {
                let client = create_cloud_client(cmd.common.deployment_env_id.as_deref()).await?;
                cmd.run(client).await
            }
            KeyValueCommand::Set(cmd) => {
                let client = create_cloud_client(cmd.common.deployment_env_id.as_deref()).await?;
                cmd.run(client).await
            }
            KeyValueCommand::Rename(cmd) => {
                let client = create_cloud_client(cmd.common.deployment_env_id.as_deref()).await?;
                cmd.run(client).await
            }
        }
    }
}

impl CreateCommand {
    pub async fn run(&self, client: impl CloudClientInterface) -> Result<()> {
        let list = client
            .get_key_value_stores(None)
            .await
            .with_context(|| format!("Error listing key value stores '{}'", self.name))?;
        if list.iter().any(|kv| kv.name == self.name) {
            bail!(r#"Key value store "{}" already exists"#, self.name)
        }
        client
            .create_key_value_store(&self.name, None)
            .await
            .with_context(|| format!("Error creating key value store '{}'", self.name))?;
        println!(r#"Key value store "{}" created"#, self.name);
        Ok(())
    }
}

impl DeleteCommand {
    pub async fn run(&self, client: impl CloudClientInterface) -> Result<()> {
        let list = client
            .get_key_value_stores(None)
            .await
            .with_context(|| format!("Error listing key value stores '{}'", self.name))?;
        let kv = list
            .iter()
            .find(|kv| kv.name == self.name)
            .with_context(|| format!("No key value store found with name \"{}\"", self.name))?;
        if self.yes || prompt_delete_resource(&self.name, &kv.links, ResourceType::KeyValueStore)? {
            client
                .delete_key_value_store(&self.name)
                .await
                .with_context(|| format!("Problem deleting key value store '{}'", self.name))?;
            println!("Key value store \"{}\" deleted", self.name);
        }
        Ok(())
    }
}

impl ListCommand {
    pub async fn run(&self, client: impl CloudClientInterface) -> Result<()> {
        if let (ListFormat::Json, Some(_)) = (&self.format, self.group_by) {
            bail!("Grouping is not supported with JSON format output")
        }
        let key_value_stores = client
            .get_key_value_stores(None)
            .await
            .with_context(|| "Error listing key value stores")?;

        if key_value_stores.is_empty() {
            println!("No key value stores found");
            return Ok(());
        }
        let resource_links = key_value_stores
            .into_iter()
            .map(|kv| ResourceLinks::new(kv.name, kv.links))
            .collect();
        match self.format {
            ListFormat::Json => print_json(
                resource_links,
                self.app.as_deref(),
                ResourceType::KeyValueStore,
            ),
            ListFormat::Table => print_table(
                resource_links,
                self.app.as_deref(),
                self.group_by.map(Into::into),
                ResourceType::KeyValueStore,
            ),
        }
    }
}

impl SetCommand {
    pub async fn run(&self, client: impl CloudClientInterface) -> Result<()> {
        let target = ResourceTarget::from_inputs(&self.store, &self.label, &self.app)?;
        let stores = client
            .get_key_value_stores(None)
            .await
            .context("Problem fetching key value stores")?;
        let store = target
            .find_in(to_resource_links(stores), ResourceType::KeyValueStore)?
            .name;
        for (key, value) in &self.key_values {
            client
                .add_key_value_pair(None, store.clone(), key.clone(), value.clone())
                .await
                .with_context(|| {
                    format!(
                        "Error adding key value pair '{key}={value}' to store '{}'",
                        store
                    )
                })?;
        }
        Ok(())
    }
}

impl RenameCommand {
    pub async fn run(&self, client: impl CloudClientInterface) -> Result<()> {
        let list = client
            .get_key_value_stores(None)
            .await
            .with_context(|| format!("Error listing key value stores '{}'", self.name))?;
        let found = list.iter().any(|kv| kv.name == self.name);
        if !found {
            bail!("No key value store found with name \"{}\"", self.name);
        }
        client
            .rename_key_value_store(&self.name, &self.new_name)
            .await?;
        println!(
            "Key value store \"{}\" is now named \"{}\"",
            self.name, self.new_name
        );
        Ok(())
    }
}

fn to_resource_links(stores: Vec<KeyValueStoreItem>) -> Vec<ResourceLinks> {
    stores
        .into_iter()
        .map(|s| ResourceLinks::new(s.name, s.links))
        .collect()
}

#[cfg(test)]
mod key_value_tests {
    use super::*;
    use cloud::MockCloudClientInterface;
    use cloud_openapi::models::KeyValueStoreItem;

    #[tokio::test]
    async fn test_create_if_store_already_exists_then_error() -> Result<()> {
        let command = CreateCommand {
            name: "kv1".to_string(),
            common: Default::default(),
        };
        let stores = vec![
            KeyValueStoreItem::new("kv1".to_string(), vec![]),
            KeyValueStoreItem::new("kv2".to_string(), vec![]),
        ];

        let mut mock = MockCloudClientInterface::new();
        mock.expect_get_key_value_stores()
            .return_once(move |_| Ok(stores));

        let result = command.run(mock).await;
        assert_eq!(
            result.unwrap_err().to_string(),
            r#"Key value store "kv1" already exists"#
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_create_if_store_does_not_exist_store_is_created() -> Result<()> {
        let command = CreateCommand {
            name: "kv1".to_string(),
            common: Default::default(),
        };
        let dbs = vec![KeyValueStoreItem::new("kv2".to_string(), vec![])];

        let mut mock = MockCloudClientInterface::new();
        mock.expect_get_key_value_stores()
            .return_once(move |_| Ok(dbs));
        mock.expect_create_key_value_store()
            .withf(move |db, rl| db == "kv1" && rl.is_none())
            .returning(|_, _| Ok(()));

        command.run(mock).await
    }

    #[tokio::test]
    async fn test_delete_if_store_does_not_exist_then_error() -> Result<()> {
        let command = DeleteCommand {
            name: "kv1".to_string(),
            common: Default::default(),
            yes: true,
        };

        let mut mock = MockCloudClientInterface::new();
        mock.expect_get_key_value_stores()
            .returning(move |_| Ok(vec![]));

        let result = command.run(mock).await;
        assert_eq!(
            result.unwrap_err().to_string(),
            r#"No key value store found with name "kv1""#
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_if_store_exists_then_it_is_deleted() -> Result<()> {
        let command = DeleteCommand {
            name: "kv1".to_string(),
            common: Default::default(),
            yes: true,
        };

        let mut mock = MockCloudClientInterface::new();
        mock.expect_get_key_value_stores()
            .returning(move |_| Ok(vec![KeyValueStoreItem::new("kv1".to_string(), vec![])]));
        mock.expect_delete_key_value_store().returning(|_| Ok(()));

        command.run(mock).await
    }
}
