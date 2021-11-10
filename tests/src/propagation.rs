// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

#![allow(unused_imports)]
use skywalking::context::propagation::*;

#[test]
fn basic() {
    let data = "1-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=-ZXhhbXBsZS5jb206ODA4MA==";
    let decoder = ContextDecoder::new(data);
    let res = decoder.decode().unwrap();

    assert_eq!(res.do_sample, true);
    assert_eq!(res.parent_trace_id, "1");
    assert_eq!(res.parent_trace_segment_id, "5");
    assert_eq!(res.parent_span_id, 3);
    assert_eq!(res.parent_service, "mesh");
    assert_eq!(res.parent_service_instance, "instance");
    assert_eq!(res.destination_endpoint, "/api/v1/health");
    assert_eq!(res.destination_address, "example.com:8080");
}

#[test]
fn less_field() {
    let data = "1-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=";
    let decoder = ContextDecoder::new(data);
    let res = decoder.decode();

    assert_eq!(res.is_err(), true);
}

#[test]
fn more_field() {
    let data = "1-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=-ZXhhbXBsZS5jb206ODA4MA==-hogehoge";
    let decoder = ContextDecoder::new(data);
    let res = decoder.decode();

    assert_eq!(res.is_err(), true);
}

#[test]
fn invalid_sample() {
    let data = "3-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=-ZXhhbXBsZS5jb206ODA4MA==";
    let decoder = ContextDecoder::new(data);
    let res = decoder.decode();

    assert_eq!(res.is_err(), true);
}
