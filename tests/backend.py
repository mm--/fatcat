
import os
import json
import pytest
import fatcat
import fatcat.sql
from fatcat.models import *
import unittest
import tempfile

## Helpers ##################################################################

def check_entity_fields(e):
    for key in ('rev', 'is_live', 'redirect_id'):
        assert key in e
    for key in ('id',):
        assert e[key] is not None

def check_release(e):
    for key in ('work', 'release_type'):
        assert key in e
    for key in ('title'):
        assert e[key] is not None
    for key in ('refs', 'creators'):
        assert type(e[key]) == list

def check_creator(e):
    for key in ('name',):
        assert e[key] is not None

def check_container(e):
    for key in ('name',):
        assert e[key] is not None

def check_file(e):
    for key in ('size', 'sha1'):
        assert e[key] is not None

@pytest.fixture
def app():
    fatcat.app.config['SQLALCHEMY_DATABASE_URI'] = 'sqlite://'
    fatcat.app.testing = True
    fatcat.app.debug = True
    fatcat.db.session.remove()
    fatcat.db.drop_all()
    fatcat.db.create_all()
    fatcat.sql.populate_db()
    return fatcat.app.test_client()


## Model Tests ###############################################################

def test_example_works(app):
    fatcat.dummy.insert_example_works()

def test_random_works(app):
    fatcat.dummy.insert_random_works()

def test_load_crossref(app):
    with open('./tests/files/crossref-works.2018-01-21.badsample.json', 'r') as f:
        raw = [json.loads(l) for l in f.readlines() if len(l) > 3]
    for obj in raw:
        fatcat.sql.add_crossref_via_model(obj)

def test_schema_release_rev(app):
    assert ReleaseRev.query.count() == 0
    e = {
        "title": "Bogus title",
        "release_type": "book",
        "creators": [],
        "refs": [],
    }
    model = release_rev_schema.load(e)
    fatcat.db.session.add(model.data)
    fatcat.db.session.commit()
    assert ReleaseRev.query.count() == 1
    model_after = ReleaseRev.query.first()
    serial = release_rev_schema.dump(model_after).data
    #check_release(serial)
    for k in e.keys():
        assert e[k] == serial[k]

def test_schema_creator_rev(app):
    assert ReleaseRev.query.count() == 0
    e = {
        "name": "Robin (Batman)",
    }
    model = creator_rev_schema.load(e)
    fatcat.db.session.add(model.data)
    fatcat.db.session.commit()
    assert CreatorRev.query.count() == 1
    model_after = CreatorRev.query.first()
    serial = creator_rev_schema.dump(model_after).data
    check_creator(serial)
    for k in e.keys():
        assert e[k] == serial[k]

def test_schema_container_rev(app):
    assert ReleaseRev.query.count() == 0
    e = {
        "name": "Papers Monthly",
    }
    model = container_rev_schema.load(e)
    fatcat.db.session.add(model.data)
    fatcat.db.session.commit()
    assert ContainerRev.query.count() == 1
    model_after = ContainerRev.query.first()
    serial = container_rev_schema.dump(model_after).data
    check_container(serial)
    for k in e.keys():
        assert e[k] == serial[k]

def test_schema_file_rev(app):
    assert ReleaseRev.query.count() == 0
    e = {
        "sha1": "asdf",
        "size": 6,
    }
    model = file_rev_schema.load(e)
    print(model)
    fatcat.db.session.add(model.data)
    fatcat.db.session.commit()
    assert FileRev.query.count() == 1
    model_after = FileRev.query.first()
    serial = file_rev_schema.dump(model_after).data
    check_file(serial)
    for k in e.keys():
        assert e[k] == serial[k]

## API Tests #################################################################

def test_health(app):
    rv = app.get('/health')
    obj = json.loads(rv.data.decode('utf-8'))
    assert obj['ok']

def test_api_work(app):
    fatcat.dummy.insert_example_works()

    # Invalid Id
    rv = app.get('/v0/work/_')
    assert rv.status_code == 404

    # Random
    rv = app.get('/v0/work/random')
    rv = app.get(rv.location)
    work = json.loads(rv.data.decode('utf-8'))
    check_entity_fields(work)
    print(work)
    assert work['title']
    assert work['work_type']

    # Valid Id (from random above)
    rv = app.get('/v0/work/{}'.format(work['id']))
    assert rv.status_code == 200

    # Missing Id
    rv = app.get('/v0/work/r3zga5b9cd7ef8gh084714iljk')
    assert rv.status_code == 404

def test_api_work_create(app):
    assert WorkIdent.query.count() == 0
    assert WorkRev.query.count() == 0
    assert WorkEdit.query.count() == 0
    assert ExtraJson.query.count() == 0
    rv = app.post('/v0/work',
        data=json.dumps(dict(title="dummy", work_type="thing", extra=dict(a=1, b="zing"))),
        headers={"content-type": "application/json"})
    print(rv)
    assert rv.status_code == 200
    assert WorkIdent.query.count() == 1
    assert WorkRev.query.count() == 1
    assert WorkEdit.query.count() == 1
    assert ExtraJson.query.count() == 1
    # not alive yet
    assert WorkIdent.query.filter(WorkIdent.is_live==True).count() == 0

def test_api_rich_create(app):

    # TODO: create user?

    rv = app.post('/v0/editgroup',
        data=json.dumps(dict(
            extra=dict(q=1, u="zing"))),
        headers={"content-type": "application/json"})
    assert rv.status_code == 200
    obj = json.loads(rv.data.decode('utf-8'))
    editgroup_id = obj['id']

    for cls in (WorkIdent, WorkRev, WorkEdit,
                ContainerIdent, ContainerRev, ContainerEdit,
                CreatorIdent, CreatorRev, CreatorEdit,
                ReleaseIdent, ReleaseRev, ReleaseEdit,
                FileIdent, FileRev, FileEdit,
                ChangelogEntry):
        assert cls.query.count() == 0

    rv = app.post('/v0/container',
        data=json.dumps(dict(
            name="schmournal",
            publisher="society of authors",
            issn="2222-3333",
            editgroup=editgroup_id,
            extra=dict(a=2, i="zing"))),
        headers={"content-type": "application/json"})
    assert rv.status_code == 200
    obj = json.loads(rv.data.decode('utf-8'))
    container_id = obj['id']

    rv = app.post('/v0/creator',
        data=json.dumps(dict(
            name="anon y. mouse",
            orcid="0000-0002-1825-0097",
            editgroup=editgroup_id,
            extra=dict(w=1, q="zing"))),
        headers={"content-type": "application/json"})
    assert rv.status_code == 200
    obj = json.loads(rv.data.decode('utf-8'))
    creator_id = obj['id']

    rv = app.post('/v0/work',
        data=json.dumps(dict(
            title="dummy work",
            work_type="book",
            editgroup=editgroup_id,
            extra=dict(a=3, b="zing"))),
        headers={"content-type": "application/json"})
    assert rv.status_code == 200
    obj = json.loads(rv.data.decode('utf-8'))
    work_id = obj['id']

    # this stub work will be referenced
    rv = app.post('/v0/release',
        data=json.dumps(dict(
            title="derivative work",
            work_type="journal-article",
            work=work_id,
            creators=[creator_id],
            doi="10.1234/58",
            editgroup=editgroup_id,
            refs=[
                dict(stub="some other journal article"),
            ],
            extra=dict(f=7, b="zing"))),
        headers={"content-type": "application/json"})
    assert rv.status_code == 200
    obj = json.loads(rv.data.decode('utf-8'))
    stub_release_id = obj['id']

    rv = app.post('/v0/release',
        data=json.dumps(dict(
            title="dummy work",
            work_type="book",
            work=work_id,
            container=container_id,
            creators=[creator_id],
            doi="10.1234/5678",
            editgroup=editgroup_id,
            refs=[
                dict(stub="some book", target=stub_release_id),
            ],
            extra=dict(f=7, b="loopy"))),
        headers={"content-type": "application/json"})
    assert rv.status_code == 200
    obj = json.loads(rv.data.decode('utf-8'))
    release_id = obj['id']

    rv = app.post('/v0/file',
        data=json.dumps(dict(
            sha1="deadbeefdeadbeef",
            size=1234,
            releases=[release_id],
            editgroup=editgroup_id,
            extra=dict(f=4, b="zing"))),
        headers={"content-type": "application/json"})
    assert rv.status_code == 200
    obj = json.loads(rv.data.decode('utf-8'))
    file_id = obj['id']

    for cls in (WorkIdent, WorkRev, WorkEdit,
                ContainerIdent, ContainerRev, ContainerEdit,
                CreatorIdent, CreatorRev, CreatorEdit,
                FileIdent, FileRev, FileEdit):
        assert cls.query.count() == 1
    for cls in (ReleaseIdent, ReleaseRev, ReleaseEdit):
        assert cls.query.count() == 2

    for cls in (WorkIdent,
                ContainerIdent,
                CreatorIdent,
                ReleaseIdent,
                FileIdent):
        assert cls.query.filter(cls.is_live==True).count() == 0

    assert ChangelogEntry.query.count() == 0
    rv = app.post('/v0/editgroup/{}/accept'.format(editgroup_id),
        headers={"content-type": "application/json"})
    assert rv.status_code == 200
    assert ChangelogEntry.query.count() == 1

    for cls in (WorkIdent, WorkRev, WorkEdit,
                ContainerIdent, ContainerRev, ContainerEdit,
                CreatorIdent, CreatorRev, CreatorEdit,
                FileIdent, FileRev, FileEdit):
        assert cls.query.count() == 1
    for cls in (ReleaseIdent, ReleaseRev, ReleaseEdit):
        assert cls.query.count() == 2

    for cls in (WorkIdent,
                ContainerIdent,
                CreatorIdent,
                FileIdent):
        assert cls.query.filter(cls.is_live==True).count() == 1
    assert ReleaseIdent.query.filter(ReleaseIdent.is_live==True).count() == 2

    # Test that foreign key relations worked
    release_rv = json.loads(app.get('/v0/release/{}'.format(release_id)).data.decode('utf-8'))
    print(release_rv)
    assert(release_rv['creators'][0]['creator'] == creator_id)
    assert(release_rv['container']['id'] == container_id)
    assert(release_rv['work']['id'] == work_id)
    assert(release_rv['refs'][0]['target'] == stub_release_id)

    file_rv = json.loads(app.get('/v0/file/{}'.format(file_id)).data.decode('utf-8'))
    print(file_rv)
    assert(file_rv['releases'][0]['release'] == release_id)
