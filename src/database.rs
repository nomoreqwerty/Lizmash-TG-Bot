use crate::profile::{Like, Location, Profile, View};
use crate::user::*;
use futures::stream::TryStreamExt;
use mongodb::bson::{doc, Document};
use mongodb::options::ClientOptions;
use std::sync::Arc;
use crate::profile;

/// Performs all database operations
///
/// Auto-removing likes and views is not implemented here, it must be done
/// in the database itself
pub struct Database {
    client: mongodb::Client,
}

impl Database {
    pub async fn init() -> Arc<Self> {
        let mut options = ClientOptions::parse("mongodb://localhost:27017/deafbot")
            .await
            .unwrap();

        options.app_name = Some(String::from("DeafBot"));

        Arc::new(Self {
            client: mongodb::Client::with_options(options).unwrap(),
        })
    }

    pub async fn add_user(&self, user: &User) {
        log::trace!("new record of the user with id `{}` created", user.id());

        self.users_collection()
            .insert_one(user, None)
            .await
            .unwrap();
    }

    pub async fn add_profile(&self, profile: &Profile) {
        log::trace!("new record of the profile with id `{}` created", profile.id());

        self.profiles_collection()
            .insert_one(profile, None)
            .await
            .unwrap();
    }

    pub async fn filter_profile(&self, filter: Document) -> Option<Profile> {
        let mut cursor = match self.profiles_collection().find(filter, None).await {
            Ok(curs) => curs,
            Err(_) => return None,
        };

        cursor.try_next().await.map_or(None, |el| el)
    }

    pub async fn set_profile_name(&self, user_id: impl Into<MyUserId>, v: &str) {
        self.profiles_collection()
            .update_one(
                doc! { "id": user_id.into() },
                doc! { "$set": { "name": v } },
                None
            )
            .await
            .unwrap();
    }

    pub async fn set_profile_age(&self, user_id: impl Into<MyUserId>, v: i64) {
        self.profiles_collection()
            .update_one(
                doc! { "id": user_id.into() },
                doc! { "$set": { "age": v } },
                None
            )
            .await
            .unwrap();
    }

    pub async fn set_profile_location(&self, user_id: impl Into<MyUserId>, v: Location) {
        self.profiles_collection()
            .update_one(
                doc! { "id": user_id.into() },
                doc! { "$set": { "location": v } },
                None
            )
            .await
            .unwrap();
    }

    pub async fn set_profile_hearing_level(&self, user_id: impl Into<MyUserId>, v: profile::HearingLevel) {
        self.profiles_collection()
            .update_one(
                doc! { "id": user_id.into() },
                doc! { "$set": { "hearing_level": v } },
                None
            )
            .await
            .unwrap();
    }

    pub async fn set_profile_description(&self, user_id: impl Into<MyUserId>, v: impl Into<Option<&str>>) {
        match v.into() {
            Some(description) => {
                self.profiles_collection()
                    .update_one(
                        doc! { "id": user_id.into() },
                        doc! { "$set": { "description": description } },
                        None
                    )
                    .await
                    .unwrap();
            }
            None => {
                self.profiles_collection()
                    .update_one(
                        doc! { "id": user_id.into() },
                        doc! { "$unset": { "description": "" } },
                        None
                    )
                    .await
                    .unwrap();
            }
        }
    }

    pub async fn set_profile_picture(&self, user_id: impl Into<MyUserId>, pics: &[String]) {
        self.profiles_collection()
            .update_one(
                doc! { "id": user_id.into() },
                doc! { "$set": { "photos": pics } },
                None
            )
            .await
            .unwrap();
    }

    pub async fn add_like(&self, like: Like) {
        self.likes_collection()
            .insert_one(like, None)
            .await
            .unwrap();
    }

    pub async fn add_view(&self, view: View) {
        self.views_collection()
            .insert_one(view, None)
            .await
            .unwrap();
    }

    /// Returns [Like] where _from_ is ID of the user who liked the user, _to_ is the specified user_id
    ///
    /// Returns _None_ if there is no such record in the database with the specified user_id
    pub async fn get_like_to_user(&self, user_id: impl Into<MyUserId>) -> Option<Like> {
        let mut cursor = match self
            .likes_collection()
            .find(doc! { "to": user_id.into() }, None)
            .await
        {
            Ok(curs) => curs,
            Err(_) => return None,
        };

        cursor.try_next().await.map_or(None, |el| el)
    }

    pub async fn find_like(
        &self,
        from: impl Into<MyUserId>,
        to: impl Into<MyUserId>,
    ) -> Option<Like> {
        self.likes_collection()
            .find_one(
                doc! {
                    "$and": [
                        { "from": from.into() },
                        { "to": to.into() }
                    ]
                },
                None,
            )
            .await
            .unwrap()
    }

    pub async fn remove_like(&self, from: impl Into<MyUserId>, to: impl Into<MyUserId>) {
        self.likes_collection()
            .delete_one(
                doc! {
                    "$and": [
                        { "from": from.into() },
                        { "to": to.into() }
                    ]
                },
                None,
            )
            .await
            .unwrap();
    }

    pub async fn get_user_views(&self, user_id: impl Into<MyUserId>) -> Vec<View> {
        let mut list: Vec<View> = vec![];
        let mut cursor = self
            .views_collection()
            .find(doc! { "from": user_id.into() }, None)
            .await
            .unwrap();
        while let Ok(Some(view)) = cursor.try_next().await {
            list.push(view);
        }
        list
    }

    pub async fn get_user(&self, id: impl Into<MyUserId>) -> Option<User> {
        let mut cursor = match self
            .users_collection()
            .find(doc! { "id": id.into() }, None)
            .await
        {
            Ok(curs) => curs,
            Err(_) => return None,
        };

        cursor.try_next().await.map_or(None, |el| el)
    }

    pub async fn get_profile(&self, id: impl Into<MyUserId>) -> Option<Profile> {
        let mut cursor = match self
            .profiles_collection()
            .find(doc! { "id": id.into() }, None)
            .await
        {
            Ok(curs) => curs,
            Err(_) => return None,
        };

        cursor.try_next().await.map_or(None, |el| el)
    }

    #[inline]
    fn users_collection(&self) -> mongodb::Collection<User> {
        log::trace!("users collection access requested");
        self.local().collection("users")
    }

    #[inline]
    fn profiles_collection(&self) -> mongodb::Collection<Profile> {
        log::trace!("profiles collection access requested");
        self.local().collection("profiles")
    }

    #[inline]
    fn likes_collection(&self) -> mongodb::Collection<Like> {
        log::trace!("likes collection access requested");
        self.local().collection("likes")
    }

    #[inline]
    fn views_collection(&self) -> mongodb::Collection<View> {
        log::trace!("views collection access requested");
        self.local().collection("views")
    }

    #[inline]
    fn local(&self) -> mongodb::Database {
        self.client.database("deafbot")
    }
}
