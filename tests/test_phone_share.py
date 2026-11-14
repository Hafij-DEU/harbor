from harbor.files_sidebar import phone_share_addr


def test_phone_share_addr():
    assert phone_share_addr("ftp://harbor@192.168.1.101:2121/") == ("192.168.1.101", 2121)
    assert phone_share_addr("ftp://harbor@10.0.0.2/") == ("10.0.0.2", 21)
    assert phone_share_addr("ftp://bob@192.168.1.101:2121/") is None
    assert phone_share_addr("smb://harbor@host/share") is None
    assert phone_share_addr("file:///home") is None
    assert phone_share_addr("ftp://harbor@host:99999/") is None
    assert phone_share_addr(None) is None
