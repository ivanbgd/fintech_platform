use fintech_common::errors::AccountingError;
use warp::reject::Reject;

#[derive(Debug)]
pub struct WebServiceAccountingError(pub AccountingError);

impl Reject for WebServiceAccountingError {}

#[derive(Debug)]
pub struct WebServiceStringError(pub String);

impl Reject for WebServiceStringError {}
