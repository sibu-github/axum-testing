use mockall::automock;
use mongodb::{
    bson::{Bson, Document},
    error::Result as MongoResult,
    options::{ClientOptions, FindOneOptions, InsertOneOptions},
    Client,
};
use serde::{de::DeserializeOwned, Serialize};

pub const DB_NAME: &str = "myDB";

#[derive(Debug, Clone)]
pub struct InsertOneResult {
    pub inserted_id: String,
}

// struct to hold the dabatabse client that can be used in the application
#[derive(Debug, Clone)]
pub struct AppDatabase(Client);

#[automock]
impl AppDatabase {
    // create new Mongo DB client and instantiate AppDatabase
    pub async fn new(uri: &str) -> MongoResult<Self> {
        let client_options = ClientOptions::parse(uri).await?;
        let client = Client::with_options(client_options)?;
        Ok(Self(client))
    }

    pub async fn find_one<T>(
        &self,
        db: &str,
        coll: &str,
        filter: Option<Document>,
        options: Option<FindOneOptions>,
    ) -> MongoResult<Option<T>>
    where
        T: DeserializeOwned + Unpin + Send + Sync + 'static,
    {
        let collection = self.0.database(db).collection::<T>(coll);
        collection.find_one(filter, options).await
    }

    pub async fn insert_one<T>(
        &self,
        db: &str,
        coll: &str,
        doc: &T,
        options: Option<InsertOneOptions>,
    ) -> MongoResult<InsertOneResult>
    where
        T: Serialize + 'static,
    {
        let collection = self.0.database(db).collection::<T>(coll);
        let result = collection.insert_one(doc, options).await?;
        let result = if let Bson::ObjectId(oid) = result.inserted_id {
            InsertOneResult {
                inserted_id: oid.to_hex(),
            }
        } else {
            InsertOneResult {
                inserted_id: String::new(),
            }
        };
        Ok(result)
    }
}
